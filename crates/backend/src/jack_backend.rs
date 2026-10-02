use crate::{output_ports, plan_connections, PlanarRenderer, Routing};
use engine::EngineProcessor;
use engine::OutputMode;
use jack::{
    AudioOut, Client, ClientOptions, Control, Frames, MidiIn, MidiOut, Port, PortFlags, PortSpec,
    ProcessScope, RawMidi,
};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};
use std::sync::Arc;

/// Headroom reserved before JACK tells us the real buffer size.
const INITIAL_FRAMES: usize = 4096;
/// MIDI messages buffered each way between the audio thread and the UI. A controller that
/// outruns this is dropping the oldest of a flood, which is the right thing to lose.
const MIDI_QUEUE: usize = 256;
/// Ports named like these are clocks and transport, never controllers worth connecting.
const MIDI_PORT_SKIP: [&str; 2] = ["through", "midi through"];

/// A JACK client that has connected to the server but isn't processing yet.
/// Opening first lets the caller learn the session sample rate before loading tracks.
pub struct JackBackend {
    client: Client,
}

struct Process {
    ports: [Port<AudioOut>; 4],
    /// How the server scheduled this thread, packed, sampled on the first callback. The
    /// server owns the thread, so looking is the only thing available.
    sched: Arc<AtomicU32>,
    renderer: PlanarRenderer,
    processor: EngineProcessor,
    midi_in: Port<MidiIn>,
    midi_out: Port<MidiOut>,
    /// Controller messages on their way to the UI thread.
    from_device: Producer<[u8; 3]>,
    /// LED updates on their way out to the controller.
    to_device: Consumer<[u8; 3]>,
}

impl jack::ProcessHandler for Process {
    fn process(&mut self, _: &Client, ps: &ProcessScope) -> Control {
        // Once, on the first callback. The server owns this thread, but it lives in our
        // process, so when the server left it ordinary we can still ask: pipewire-jack does
        // not always elevate a client's callback thread even where the server's own loops are
        // realtime. Never override a server that already granted it, since its choice of
        // priority is deliberate and ours would be lower.
        if self.sched.load(Relaxed) == 0 {
            let mut got = crate::realtime::scheduling();
            if !got.is_realtime() {
                if let Ok(raised) =
                    crate::realtime::request_realtime(crate::realtime::AUDIO_PRIORITY)
                {
                    got = raised;
                }
            }
            self.sched.store(got.to_bits(), Relaxed);
        }
        for event in self.midi_in.iter(ps) {
            // Only the three-byte channel messages the mapping layer reads.
            if let [a, b, c] = *event.bytes {
                let _ = self.from_device.push([a, b, c]);
            }
        }
        let mut writer = self.midi_out.writer(ps);
        while let Ok(bytes) = self.to_device.pop() {
            let _ = writer.write(&RawMidi {
                time: 0,
                bytes: &bytes,
            });
        }
        let [a, b, c, d] = &mut self.ports;
        self.renderer.render(
            &mut self.processor,
            [
                a.as_mut_slice(ps),
                b.as_mut_slice(ps),
                c.as_mut_slice(ps),
                d.as_mut_slice(ps),
            ],
        );
        Control::Continue
    }

    fn buffer_size(&mut self, _: &Client, size: Frames) -> Control {
        self.renderer.reserve(size as usize);
        Control::Continue
    }
}

struct Notifications {
    xruns: Arc<AtomicU64>,
}

impl jack::NotificationHandler for Notifications {
    fn xrun(&mut self, _: &Client) -> Control {
        self.xruns.fetch_add(1, Relaxed);
        Control::Continue
    }
}

/// An active JACK client. Dropping it (or calling `stop`) disconnects from the server.
pub struct Running {
    client: jack::AsyncClient<Notifications, Process>,
    name: String,
    /// Controller messages the audio thread has picked up.
    midi_rx: Consumer<[u8; 3]>,
    /// LED updates waiting to go out.
    midi_tx: Producer<[u8; 3]>,
    /// Controller ports already connected, so a rescan only connects what is new.
    midi_connected: Vec<String>,
    warnings: Vec<String>,
    xruns: Arc<AtomicU64>,
    /// What the callback reported about its own scheduling, once it has run.
    sched: Arc<AtomicU32>,
    /// What our four output ports carry, which is what names them.
    output: OutputMode,
    sample_rate: u32,
    buffer_size: u32,
}

impl JackBackend {
    /// Connect to a running JACK or PipeWire-JACK server. Never starts a server.
    pub fn open(client_name: &str) -> Result<Self, String> {
        let (client, _status) = Client::new(client_name, ClientOptions::NO_START_SERVER)
            .map_err(|e| format!("cannot connect to JACK: {e}"))?;
        Ok(Self { client })
    }

    pub fn sample_rate(&self) -> u32 {
        self.client.sample_rate()
    }

    pub fn activate(
        self,
        processor: EngineProcessor,
        routing: &Routing,
        output: OutputMode,
    ) -> Result<Running, String> {
        let client = self.client;
        let reg = |name: &str| {
            client
                .register_port(name, AudioOut::default())
                .map_err(|e| format!("register {name}: {e}"))
        };
        let names = output_ports(output);
        let ports = [
            reg(names[0])?,
            reg(names[1])?,
            reg(names[2])?,
            reg(names[3])?,
        ];
        let midi_in = client
            .register_port("midi_in", MidiIn::default())
            .map_err(|e| format!("register midi_in: {e}"))?;
        let midi_out = client
            .register_port("midi_out", MidiOut::default())
            .map_err(|e| format!("register midi_out: {e}"))?;
        let (from_device, midi_rx) = RingBuffer::new(MIDI_QUEUE);
        let (midi_tx, to_device) = RingBuffer::new(MIDI_QUEUE);
        let frames = (client.buffer_size() as usize).max(INITIAL_FRAMES);
        let sample_rate = client.sample_rate();
        let buffer_size = client.buffer_size();
        let name = client.name().to_string();
        let xruns = Arc::new(AtomicU64::new(0));
        let sched = Arc::new(AtomicU32::new(0));

        let process = Process {
            sched: sched.clone(),
            ports,
            renderer: PlanarRenderer::new(frames).split_mono(*routing == Routing::Split),
            processor,
            midi_in,
            midi_out,
            from_device,
            to_device,
        };
        let active = client
            .activate_async(
                Notifications {
                    xruns: xruns.clone(),
                },
                process,
            )
            .map_err(|e| format!("activate: {e}"))?;

        let physical = active.as_client().ports(
            None,
            Some(AudioOut::default().jack_port_type()),
            PortFlags::IS_INPUT | PortFlags::IS_PHYSICAL,
        );
        let plan = plan_connections(&name, routing, &physical, output);
        let mut warnings = plan.warnings;
        for (ours, theirs) in &plan.connections {
            if let Err(e) = active.as_client().connect_ports_by_name(ours, theirs) {
                warnings.push(format!("connect {ours} -> {theirs}: {e}"));
            }
        }
        Ok(Running {
            client: active,
            name,
            warnings,
            xruns,
            sched,
            sample_rate,
            buffer_size,
            midi_rx,
            midi_tx,
            midi_connected: Vec::new(),
            output,
        })
    }
}

impl Running {
    /// Take whatever the controller has sent since the last call.
    pub fn take_midi(&mut self) -> Vec<[u8; 3]> {
        let mut out = Vec::new();
        while let Ok(bytes) = self.midi_rx.pop() {
            out.push(bytes);
        }
        out
    }

    /// Queue three bytes for the controller. Dropped if the queue is full, since an LED
    /// update is only worth as much as the next one.
    pub fn send_midi(&mut self, bytes: [u8; 3]) {
        let _ = self.midi_tx.push(bytes);
    }

    /// Connect any controller port matching `wanted` that is not connected already, and
    /// report what was newly connected. Called on a timer, this is hotplug.
    pub fn connect_midi(&mut self, wanted: &[String]) -> Vec<String> {
        let client = self.client.as_client();
        let ports = client.ports(None, Some("8 bit raw midi"), PortFlags::IS_OUTPUT);
        let mut connected = Vec::new();
        for port in ports {
            let lower = port.to_ascii_lowercase();
            if lower.starts_with(&self.name.to_ascii_lowercase())
                || MIDI_PORT_SKIP.iter().any(|skip| lower.contains(skip))
                || self.midi_connected.contains(&port)
            {
                continue;
            }
            let matches = wanted.is_empty()
                || wanted
                    .iter()
                    .any(|want| lower.contains(&want.to_ascii_lowercase()));
            if !matches {
                continue;
            }
            let ours = format!("{}:midi_in", self.name);
            match client.connect_ports_by_name(&port, &ours) {
                Ok(()) => {
                    self.midi_connected.push(port.clone());
                    connected.push(port);
                }
                Err(e) => self.warnings.push(format!("connect {port}: {e}")),
            }
        }
        // Forget ports that have gone away, so replugging reconnects them.
        let live = client.ports(None, Some("8 bit raw midi"), PortFlags::IS_OUTPUT);
        self.midi_connected.retain(|p| live.contains(p));
        connected
    }

    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    pub fn client_name(&self) -> &str {
        &self.name
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn buffer_size(&self) -> u32 {
        self.buffer_size
    }

    pub fn xruns(&self) -> u64 {
        self.xruns.load(Relaxed)
    }

    /// How the server scheduled the audio callback, or `None` before it has run once. The
    /// server owns that thread, so this is a report and not something dj-tui can change: a
    /// callback that is not realtime glitches under load however large the buffer is.
    pub fn scheduling(&self) -> Option<crate::realtime::Scheduling> {
        crate::realtime::Scheduling::from_bits(self.sched.load(Relaxed))
    }

    /// Current (ours, theirs) connections, read back from the server.
    pub fn connected_ports(&self) -> Vec<(String, String)> {
        let client = self.client.as_client();
        output_ports(self.output)
            .iter()
            .map(|p| format!("{}:{p}", self.name))
            .filter_map(|full| client.port_by_name(&full).map(|port| (full, port)))
            .flat_map(|(full, port)| {
                port.get_connections()
                    .into_iter()
                    .map(move |other| (full.clone(), other))
            })
            .collect()
    }

    pub fn stop(self) {
        let _ = self.client.deactivate();
    }
}

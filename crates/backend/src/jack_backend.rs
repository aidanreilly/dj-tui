use crate::{plan_connections, PlanarRenderer, Routing, OUTPUT_PORTS};
use engine::EngineProcessor;
use jack::{
    AudioOut, Client, ClientOptions, Control, Frames, Port, PortFlags, PortSpec, ProcessScope,
};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::Arc;

/// Headroom reserved before JACK tells us the real buffer size.
const INITIAL_FRAMES: usize = 4096;

/// A JACK client that has connected to the server but isn't processing yet.
/// Opening first lets the caller learn the session sample rate before loading tracks.
pub struct JackBackend {
    client: Client,
}

struct Process {
    ports: [Port<AudioOut>; 4],
    renderer: PlanarRenderer,
    processor: EngineProcessor,
}

impl jack::ProcessHandler for Process {
    fn process(&mut self, _: &Client, ps: &ProcessScope) -> Control {
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
    warnings: Vec<String>,
    xruns: Arc<AtomicU64>,
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
    ) -> Result<Running, String> {
        let client = self.client;
        let reg = |name: &str| {
            client
                .register_port(name, AudioOut::default())
                .map_err(|e| format!("register {name}: {e}"))
        };
        let ports = [
            reg(OUTPUT_PORTS[0])?,
            reg(OUTPUT_PORTS[1])?,
            reg(OUTPUT_PORTS[2])?,
            reg(OUTPUT_PORTS[3])?,
        ];
        let frames = (client.buffer_size() as usize).max(INITIAL_FRAMES);
        let sample_rate = client.sample_rate();
        let buffer_size = client.buffer_size();
        let name = client.name().to_string();
        let xruns = Arc::new(AtomicU64::new(0));

        let process = Process {
            ports,
            renderer: PlanarRenderer::new(frames).split_mono(*routing == Routing::Split),
            processor,
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
        let plan = plan_connections(&name, routing, &physical);
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
            sample_rate,
            buffer_size,
        })
    }
}

impl Running {
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

    /// Current (ours, theirs) connections, read back from the server.
    pub fn connected_ports(&self) -> Vec<(String, String)> {
        let client = self.client.as_client();
        OUTPUT_PORTS
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

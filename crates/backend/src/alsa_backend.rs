//! Playback straight to an ALSA device, for running without a sound server.
//!
//! The device is driven from a thread of our own: fill a period, hand it over, wait for room.
//! Memory-mapped transfer is asked for first and falls back to the copying path, since the
//! plugin devices (`default`, `pulse`, anything routed through PipeWire) rarely offer mmap.

use crate::{PlanarRenderer, Routing};
use alsa::device_name::HintIter;
use alsa::pcm::{Access, Format, HwParams, State, PCM};
use alsa::{Direction, ValueOr};
use engine::EngineProcessor;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed};
use std::sync::Arc;

/// Periods in the ring. Two is the minimum that lets one play while the other is filled.
const PERIODS: u32 = 3;
/// Channels wanted: master left and right, then the headphone cue pair.
const WANTED_CHANNELS: u32 = 4;

/// A playback device as the setup screen lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    /// What to open, such as `hw:0,0` or `default`.
    pub name: String,
    /// What to show a person.
    pub description: String,
    /// True for a card addressed directly rather than through a plugin.
    pub hardware: bool,
}

/// Playback devices ALSA knows about, the direct ones first.
pub fn devices() -> Vec<Device> {
    let Ok(hints) = HintIter::new_str(None, "pcm") else {
        return Vec::new();
    };
    let mut found: Vec<Device> = Vec::new();
    for hint in hints {
        // Capture-only devices are no use for playback.
        if hint.direction == Some(Direction::Capture) {
            continue;
        }
        let Some(name) = hint.name else { continue };
        if name == "null" {
            continue;
        }
        let description = hint
            .desc
            .unwrap_or_else(|| name.clone())
            .replace('\n', ", ");
        let hardware = name.starts_with("hw:") || name.starts_with("plughw:");
        if found.iter().any(|d| d.name == name) {
            continue;
        }
        found.push(Device {
            name,
            description,
            hardware,
        });
    }
    // Hardware first, then plugins, each alphabetically, so the useful ones are at the top.
    found.sort_by(|a, b| b.hardware.cmp(&a.hardware).then(a.name.cmp(&b.name)));
    found
}

/// A device that is open but not yet playing, so the caller can read the rate it settled on
/// before loading any tracks.
pub struct AlsaBackend {
    pcm: PCM,
    device: String,
    rate: u32,
    channels: u32,
    period: u32,
    buffer: u32,
    /// True when the device gave us a memory mapping rather than a copying transfer.
    mmap: bool,
}

impl AlsaBackend {
    /// Open `device` for playback, asking for `rate` and a period of `period` frames. ALSA
    /// may settle on something else, which is what `sample_rate` and `buffer_size` report.
    ///
    /// Memory mapping is tried first and actually tested: the plugin devices advertise it in
    /// their parameters and then refuse the mapping, so the only honest check is to ask for
    /// one. When that fails the device is opened again for the copying transfer.
    pub fn open(device: &str, rate: u32, period: u32) -> Result<AlsaBackend, String> {
        if let Ok(mapped) = configure(device, rate, period, Access::MMapInterleaved) {
            if mapped.pcm.direct_mmap_playback::<f32>().is_ok() {
                return Ok(AlsaBackend {
                    mmap: true,
                    ..mapped
                });
            }
        }
        configure(device, rate, period, Access::RWInterleaved)
    }

    pub fn sample_rate(&self) -> u32 {
        self.rate
    }

    pub fn channels(&self) -> u32 {
        self.channels
    }

    /// True when the device is memory mapped rather than copied into.
    pub fn is_mmap(&self) -> bool {
        self.mmap
    }

    pub fn buffer_size(&self) -> u32 {
        self.period
    }

    /// Frames in the whole ring, which is several periods.
    pub fn ring_frames(&self) -> u32 {
        self.buffer
    }

    /// Start the playback thread.
    pub fn activate(
        self,
        processor: EngineProcessor,
        routing: &Routing,
    ) -> Result<AlsaRunning, String> {
        let AlsaBackend {
            pcm,
            device,
            rate,
            channels,
            period,
            buffer,
            mmap,
        } = self;
        let xruns = Arc::new(AtomicU64::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let split = *routing == Routing::Split || channels < 4;
        let renderer = PlanarRenderer::new(period as usize).split_mono(split && channels < 4);
        let sched = Arc::new(AtomicU32::new(0));
        let thread_xruns = xruns.clone();
        let thread_stop = stop.clone();
        let thread_sched = sched.clone();
        let handle = std::thread::Builder::new()
            .name("dj-tui-alsa".into())
            .spawn(move || {
                // This thread is ours, so unlike the JACK path it can ask. A refusal is the
                // ordinary case on a machine with no realtime budget and is never fatal:
                // what it got is reported instead, on the status line.
                let got = crate::realtime::request_realtime(crate::realtime::AUDIO_PRIORITY)
                    .unwrap_or_else(|_| crate::realtime::scheduling());
                thread_sched.store(got.to_bits(), Relaxed);
                play(
                    pcm,
                    processor,
                    renderer,
                    channels as usize,
                    period as usize,
                    buffer as usize,
                    mmap,
                    &thread_xruns,
                    &thread_stop,
                )
            })
            .map_err(|e| format!("start the ALSA thread: {e}"))?;
        Ok(AlsaRunning {
            sched,
            device,
            rate,
            period,
            buffer,
            channels,
            xruns,
            stop,
            handle: Some(handle),
        })
    }
}

/// Open and configure one device for `access`, reporting what ALSA settled on.
fn configure(device: &str, rate: u32, period: u32, access: Access) -> Result<AlsaBackend, String> {
    let pcm =
        PCM::new(device, Direction::Playback, false).map_err(|e| format!("open {device}: {e}"))?;
    let (settled_rate, channels, period, buffer) = {
        let hw = HwParams::any(&pcm).map_err(|e| format!("{device}: {e}"))?;
        // Four channels if the card has them, two otherwise.
        let channels = if hw.set_channels(WANTED_CHANNELS).is_ok() {
            WANTED_CHANNELS
        } else {
            hw.set_channels(2)
                .map_err(|e| format!("{device}: no stereo output: {e}"))?;
            2
        };
        hw.set_access(access)
            .map_err(|e| format!("{device}: no interleaved access: {e}"))?;
        hw.set_format(Format::float())
            .map_err(|e| format!("{device}: no 32 bit float format: {e}"))?;
        hw.set_rate_near(rate, ValueOr::Nearest)
            .map_err(|e| format!("{device}: rate {rate}: {e}"))?;
        hw.set_period_size_near(period as i64, ValueOr::Nearest)
            .map_err(|e| format!("{device}: period {period}: {e}"))?;
        hw.set_periods(PERIODS, ValueOr::Nearest)
            .map_err(|e| format!("{device}: {PERIODS} periods: {e}"))?;
        pcm.hw_params(&hw)
            .map_err(|e| format!("{device}: apply parameters: {e}"))?;
        let settled_rate = hw.get_rate().map_err(|e| format!("{device}: {e}"))?;
        let period = hw.get_period_size().map_err(|e| format!("{device}: {e}"))? as u32;
        let buffer = hw.get_buffer_size().map_err(|e| format!("{device}: {e}"))? as u32;
        // Start only when the ring is full, and wake us as soon as one period is free.
        let sw = pcm
            .sw_params_current()
            .map_err(|e| format!("{device}: {e}"))?;
        sw.set_start_threshold(buffer as i64)
            .map_err(|e| format!("{device}: start threshold: {e}"))?;
        sw.set_avail_min(period as i64)
            .map_err(|e| format!("{device}: avail min: {e}"))?;
        pcm.sw_params(&sw)
            .map_err(|e| format!("{device}: apply software parameters: {e}"))?;
        (settled_rate, channels, period, buffer)
    };
    Ok(AlsaBackend {
        pcm,
        device: device.to_string(),
        rate: settled_rate,
        channels,
        period,
        buffer,
        mmap: false,
    })
}

/// The playback loop: render a period, hand it to the device, wait for room, repeat.
#[allow(clippy::too_many_arguments)]
fn play(
    pcm: PCM,
    mut processor: EngineProcessor,
    mut renderer: PlanarRenderer,
    channels: usize,
    period: usize,
    ring: usize,
    mmap: bool,
    xruns: &AtomicU64,
    stop: &AtomicBool,
) {
    let mut planes = [
        vec![0.0f32; period],
        vec![0.0f32; period],
        vec![0.0f32; period],
        vec![0.0f32; period],
    ];
    let mut out = vec![0.0f32; period * channels];
    // A device straight out of hw_params is in the setup state and will not take anything.
    if let Err(e) = pcm.prepare() {
        xruns.fetch_add(1, Relaxed);
        let _ = pcm.try_recover(e, true);
    }

    let mut mmap_playback = if mmap {
        match pcm.direct_mmap_playback::<f32>() {
            Ok(m) => Some(m),
            // The mapping can still be refused here, in which case nothing plays: the access
            // mode was already set, so there is no copying path left to fall back to.
            Err(e) => {
                let _ = pcm.try_recover(e, true);
                None
            }
        }
    } else {
        None
    };
    let io = if mmap { None } else { pcm.io_f32().ok() };

    // Fill the ring with silence, so the first rendered period arrives with room to spare
    // rather than against a device that has already run dry.
    let silence = vec![0.0f32; period * channels];
    for _ in 0..ring.div_ceil(period.max(1)) {
        send(&pcm, &mut mmap_playback, &io, &silence, channels, xruns);
    }

    while !stop.load(Relaxed) {
        {
            let [a, b, c, d] = &mut planes;
            renderer.render(
                &mut processor,
                [
                    &mut a[..period],
                    &mut b[..period],
                    &mut c[..period],
                    &mut d[..period],
                ],
            );
        }
        // Master on the first pair; the cue pair only exists on a four channel device.
        for frame in 0..period {
            for ch in 0..channels {
                out[frame * channels + ch] = planes[ch.min(3)][frame];
            }
        }
        send(&pcm, &mut mmap_playback, &io, &out, channels, xruns);
    }
    let _ = pcm.drop();
}

/// Hand one period to the device, whichever way it takes it, recovering from an underrun.
fn send(
    pcm: &PCM,
    mmap: &mut Option<alsa::direct::pcm::MmapPlayback<f32>>,
    io: &Option<alsa::pcm::IO<f32>>,
    samples: &[f32],
    channels: usize,
    xruns: &AtomicU64,
) {
    let frames = samples.len() / channels.max(1);
    let result = match (mmap.as_mut(), io) {
        (Some(mmap), _) => {
            let mut sent = 0;
            let mut result = Ok(());
            while sent < frames {
                if mmap.avail() == 0 {
                    // Nothing has started yet when the ring filled during the prefill.
                    if pcm.state() == State::Prepared {
                        let _ = pcm.start();
                    }
                    if let Err(e) = pcm.wait(Some(200)) {
                        result = Err(e);
                        break;
                    }
                    continue;
                }
                let mut rest = samples[sent * channels..].iter().copied();
                sent += mmap.write(&mut rest) as usize;
            }
            result
        }
        (None, Some(io)) => io.writei(samples).map(|_| ()),
        (None, None) => Err(alsa::Error::unsupported("no way to reach the device")),
    };
    if let Err(e) = result {
        if std::env::var("DJ_TUI_ALSA_DEBUG").is_ok() {
            eprintln!("alsa: {e}, state {:?}", pcm.state());
        }
        xruns.fetch_add(1, Relaxed);
        let _ = pcm.try_recover(e, true);
    }
}

/// A device that is playing. Dropping it, or calling `stop`, closes the device.
pub struct AlsaRunning {
    device: String,
    rate: u32,
    period: u32,
    buffer: u32,
    channels: u32,
    xruns: Arc<AtomicU64>,
    /// What the playback thread got when it asked for realtime.
    sched: Arc<AtomicU32>,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl AlsaRunning {
    pub fn device(&self) -> &str {
        &self.device
    }

    pub fn sample_rate(&self) -> u32 {
        self.rate
    }

    /// Frames in one period, which is what latency is measured in.
    pub fn buffer_size(&self) -> u32 {
        self.period
    }

    /// Frames in the whole ring.
    pub fn ring_size(&self) -> u32 {
        self.buffer
    }

    pub fn channels(&self) -> u32 {
        self.channels
    }

    pub fn xruns(&self) -> u64 {
        self.xruns.load(Relaxed)
    }

    /// What the playback thread got when it asked for realtime, or `None` before it has
    /// started. Not realtime means it competes with everything else, which is heard as
    /// glitches under load whatever the buffer size is.
    pub fn scheduling(&self) -> Option<crate::realtime::Scheduling> {
        crate::realtime::Scheduling::from_bits(self.sched.load(Relaxed))
    }

    pub fn stop(mut self) {
        self.shut_down();
    }

    fn shut_down(&mut self) {
        self.stop.store(true, Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for AlsaRunning {
    fn drop(&mut self) {
        self.shut_down();
    }
}

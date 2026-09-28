//! Audio output backends. v1 has JACK (which also covers PipeWire via pipewire-jack);
//! and raw ALSA for running without a server.

mod alsa_backend;
mod jack_backend;
mod planar;
pub mod realtime;
mod routing;

pub use alsa_backend::{devices as alsa_devices, AlsaBackend, AlsaRunning, Device};
pub use jack_backend::{JackBackend, Running};
pub use planar::PlanarRenderer;
pub use routing::{plan_connections, Plan, Routing, OUTPUT_PORTS};

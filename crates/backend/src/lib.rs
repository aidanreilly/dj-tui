//! Audio output backends. v1 has JACK (which also covers PipeWire via pipewire-jack);
//! raw ALSA arrives in M10.

mod jack_backend;
mod planar;
mod routing;

pub use jack_backend::{JackBackend, Running};
pub use planar::PlanarRenderer;
pub use routing::{plan_connections, Plan, Routing, OUTPUT_PORTS};

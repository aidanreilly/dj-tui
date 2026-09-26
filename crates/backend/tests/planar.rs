//! Turning the engine's interleaved output into JACK's per-port buffers.

use backend::PlanarRenderer;
use engine::{channel, Command, DeckId::*, Engine, Track};
use std::sync::Arc;

#[test]
fn renders_master_and_cue_into_four_port_buffers() {
    let (mut h, mut p) = channel(Engine::new(), 8);
    let mut data = Vec::new();
    for _ in 0..1000 {
        data.extend_from_slice(&[0.5, -0.25]);
    }
    h.send(Command::Load(A, Arc::new(Track::from_interleaved(data, 48_000)))).unwrap();
    h.send(Command::PlayPause(A)).unwrap();
    h.send(Command::SetCrossfader(-1.0)).unwrap();
    h.send(Command::SetHeadphoneCue(A, true)).unwrap();
    h.send(Command::SetCueMix(0.0)).unwrap();

    let mut r = PlanarRenderer::new(64);
    let (mut ml, mut mr, mut cl, mut cr) = (vec![9.0; 64], vec![9.0; 64], vec![9.0; 64], vec![9.0; 64]);
    r.render(&mut p, [&mut ml, &mut mr, &mut cl, &mut cr]);
    // Constant-power centre is the default curve but the crossfader is fully on A.
    assert!(ml.iter().all(|&s| (s - 0.5).abs() < 1e-6));
    assert!(mr.iter().all(|&s| (s + 0.25).abs() < 1e-6));
    assert!(cl.iter().all(|&s| (s - 0.5).abs() < 1e-6));
    assert!(cr.iter().all(|&s| (s + 0.25).abs() < 1e-6));
    assert_eq!(h.snapshot().decks[0].position, 64.0);
}

#[test]
fn buffers_larger_than_reserved_are_silenced_not_overrun() {
    let (_h, mut p) = channel(Engine::new(), 8);
    let mut r = PlanarRenderer::new(16);
    let mut bufs: Vec<Vec<f32>> = (0..4).map(|_| vec![9.0; 32]).collect();
    let [a, b, c, d] = &mut bufs[..] else { unreachable!() };
    r.render(&mut p, [a, b, c, d]);
    assert!(bufs.iter().all(|b| b.iter().all(|&s| s == 0.0)));
}

#[test]
fn reserve_grows_capacity() {
    let (_h, mut p) = channel(Engine::new(), 8);
    let mut r = PlanarRenderer::new(16);
    r.reserve(32);
    let mut bufs: Vec<Vec<f32>> = (0..4).map(|_| vec![9.0; 32]).collect();
    let [a, b, c, d] = &mut bufs[..] else { unreachable!() };
    r.render(&mut p, [a, b, c, d]);
    assert_eq!(r.frames_rendered(), 32);
}

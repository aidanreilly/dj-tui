//! The audio path must never touch the allocator, including while applying commands.

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use engine::{channel, Command, DeckId::*, Engine, Track};
use std::sync::Arc;

#[global_allocator]
static ALLOC: AllocDisabler = AllocDisabler;

#[test]
fn process_with_commands_does_not_allocate_or_free() {
    let (mut h, mut p) = channel(Engine::new(), 32);
    let t1 = Arc::new(Track::from_interleaved(vec![0.2; 20_000], 48_000));
    let t2 = Arc::new(Track::from_interleaved(vec![0.3; 20_000], 48_000));
    let mut master = vec![0.0; 8192 * 2];
    let mut cue = vec![0.0; 8192 * 2];

    h.send(Command::Load(A, t1.clone())).unwrap();
    h.send(Command::PlayPause(A)).unwrap();
    h.send(Command::HotCue(A, 0)).unwrap();
    h.send(Command::SetHeadphoneCue(A, true)).unwrap();
    h.send(Command::SetEq(A, engine::dsp::EqBand::Mid, -6.0))
        .unwrap();
    h.send(Command::SetEqKill(A, engine::dsp::EqBand::Low, true))
        .unwrap();
    h.send(Command::SetFilter(A, -0.6)).unwrap();
    h.send(Command::SetTrim(A, 3.0)).unwrap();
    h.send(Command::SetLoop(A, Some((100.0, 5_000.0)))).unwrap();
    h.send(Command::SetBeatFrames(A, 24_000.0)).unwrap();
    h.send(Command::SetFxKind(A, engine::fx::FxKind::Echo))
        .unwrap();
    h.send(Command::SetFxWet(A, 0.8)).unwrap();
    h.send(Command::SetFxOn(A, true)).unwrap();
    h.send(Command::SetKeyLock(A, true)).unwrap();
    h.send(Command::SetRate(A, 1.08)).unwrap();
    assert_no_alloc(|| p.process(&mut master, &mut cue));

    // Replacing a track must not free the old one on this thread.
    h.send(Command::Load(A, t2.clone())).unwrap();
    h.send(Command::Load(B, t1.clone())).unwrap();
    drop(t1);
    assert_no_alloc(|| p.process(&mut master, &mut cue));
    assert_no_alloc(|| p.process(&mut master[..256], &mut cue[..256]));
    // The echo tail keeps running after the slot is switched off.
    h.send(Command::SetFxOn(A, false)).unwrap();
    assert_no_alloc(|| p.process(&mut master, &mut cue));
    h.collect_garbage();
}

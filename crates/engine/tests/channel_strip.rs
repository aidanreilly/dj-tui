//! EQ, filter, trim and meters wired into the engine through commands.

use engine::dsp::EqBand;
use engine::{channel, Command, DeckId::*, Engine, EngineHandle, EngineProcessor, Track};
use std::sync::Arc;

const FS: u32 = 48_000;

fn sine_track(freq: f32, amp: f32) -> Arc<Track> {
    let data = (0..FS as usize * 4)
        .flat_map(|i| {
            let s = amp * (std::f32::consts::TAU * freq * i as f32 / FS as f32).sin();
            [s, s]
        })
        .collect();
    Arc::new(Track::from_interleaved(data, FS))
}

fn setup(freq: f32) -> (EngineHandle, EngineProcessor) {
    let (mut h, p) = channel(Engine::with_sample_rate(FS), 64);
    for c in [
        Command::Load(A, sine_track(freq, 0.5)),
        Command::PlayPause(A),
        Command::SetCrossfader(-1.0),
        Command::SetCueMix(0.0),
    ] {
        h.send(c).unwrap();
    }
    (h, p)
}

/// Peak of master and cue over the last 100 ms after running `secs`.
fn run(p: &mut EngineProcessor, secs: f32) -> (f32, f32) {
    let (mut m, mut c) = (vec![0.0; 1024], vec![0.0; 1024]);
    let blocks = (secs * FS as f32 / 512.0) as usize;
    let tail = blocks.saturating_sub((0.1 * FS as f32 / 512.0) as usize);
    let (mut pm, mut pc) = (0f32, 0f32);
    for i in 0..blocks {
        p.process(&mut m, &mut c);
        if i >= tail {
            pm = m.iter().fold(pm, |a, s| a.max(s.abs()));
            pc = c.iter().fold(pc, |a, s| a.max(s.abs()));
        }
    }
    (pm, pc)
}

#[test]
fn low_kill_command_removes_bass_from_master() {
    let (mut h, mut p) = setup(50.0);
    let (before, _) = run(&mut p, 0.3);
    h.send(Command::SetEqKill(A, EqBand::Low, true)).unwrap();
    let (after, _) = run(&mut p, 0.5);
    assert!(before > 0.4, "{before}");
    assert!(after < 0.02, "{after}");
    h.send(Command::SetEqKill(A, EqBand::Low, false)).unwrap();
    let (restored, _) = run(&mut p, 0.5);
    assert!(restored > 0.4);
}

#[test]
fn eq_gain_command_changes_level() {
    let (mut h, mut p) = setup(8000.0);
    h.send(Command::SetEq(A, EqBand::High, -12.0)).unwrap();
    let (m, _) = run(&mut p, 0.5);
    assert!((m - 0.5 * 0.2512).abs() < 0.02, "{m}");
}

#[test]
fn filter_command_low_passes_the_channel() {
    let (mut h, mut p) = setup(6000.0);
    h.send(Command::SetFilter(A, -1.0)).unwrap();
    let (m, _) = run(&mut p, 0.5);
    assert!(m < 0.01, "{m}");
}

#[test]
fn trim_command_raises_level() {
    let (mut h, mut p) = setup(1000.0);
    h.send(Command::SetTrim(A, 6.0)).unwrap();
    let (m, _) = run(&mut p, 0.3);
    assert!((m - 0.5 * 1.995).abs() < 0.02, "{m}");
}

#[test]
fn headphone_cue_hears_eq_but_not_the_fader() {
    let (mut h, mut p) = setup(50.0);
    for c in [
        Command::SetHeadphoneCue(A, true),
        Command::SetChannelFader(A, 0.0),
        Command::SetEqKill(A, EqBand::Low, true),
    ] {
        h.send(c).unwrap();
    }
    let (m, c) = run(&mut p, 0.5);
    assert_eq!(m, 0.0);
    assert!(c < 0.02, "cue should be post-EQ: {c}");
}

#[test]
fn meters_hold_the_peak_until_taken() {
    let (mut h, mut p) = setup(1000.0);
    h.send(Command::SetChannelFader(A, 0.0)).unwrap();
    // The first block carries filter ringing from the track starting; let it pass.
    run(&mut p, 0.2);
    h.take_meters();
    run(&mut p, 0.2);
    let m = h.take_meters();
    assert!(
        (m.channels[0] - 0.5).abs() < 0.01,
        "channel meter is pre-fader: {:?}",
        m
    );
    assert_eq!(m.channels[1], 0.0);
    assert_eq!(m.master, 0.0);
    let again = h.take_meters();
    assert_eq!(again.channels[0], 0.0, "taking resets");
}

#[test]
fn master_meter_is_post_fader() {
    let (mut h, mut p) = setup(1000.0);
    h.send(Command::SetChannelFader(A, 0.5)).unwrap();
    run(&mut p, 0.2);
    h.take_meters();
    run(&mut p, 0.2);
    let m = h.take_meters();
    assert!((m.master - 0.25).abs() < 0.01, "{m:?}");
}

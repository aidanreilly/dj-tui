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

#[test]
fn the_effect_slot_sits_after_the_filter_and_before_the_fader() {
    use engine::fx::FxKind;
    let (mut h, mut p) = channel(Engine::new(), 16);
    h.send(Command::Load(A, sine_track(440.0, 0.5))).unwrap();
    h.send(Command::SetCrossfader(-1.0)).unwrap();
    h.send(Command::PlayPause(A)).unwrap();
    h.send(Command::SetFxKind(A, FxKind::Bitcrusher)).unwrap();
    h.send(Command::SetFxWet(A, 1.0)).unwrap();
    h.send(Command::SetFxOn(A, true)).unwrap();
    // The fader is down, so nothing the effect does can reach the master bus.
    h.send(Command::SetChannelFader(A, 0.0)).unwrap();
    let (mut m, mut c) = (vec![0.0; 2_048], vec![0.0; 2_048]);
    p.process(&mut m, &mut c);
    assert!(
        m.iter().all(|s| *s == 0.0),
        "the fader still has the last word"
    );

    h.send(Command::SetChannelFader(A, 1.0)).unwrap();
    p.process(&mut m, &mut c);
    assert!(
        m.iter().any(|s| *s != 0.0),
        "and lets the effect through again"
    );
}

#[test]
fn an_effect_changes_the_sound_and_switching_it_off_brings_it_back() {
    use engine::fx::FxKind;
    let render = |on: bool| {
        let (mut h, mut p) = channel(Engine::new(), 16);
        h.send(Command::Load(A, sine_track(440.0, 0.5))).unwrap();
        h.send(Command::SetCrossfader(-1.0)).unwrap();
        h.send(Command::PlayPause(A)).unwrap();
        h.send(Command::SetFxKind(A, FxKind::Bitcrusher)).unwrap();
        h.send(Command::SetFxWet(A, 1.0)).unwrap();
        h.send(Command::SetFxOn(A, on)).unwrap();
        let (mut m, mut c) = (vec![0.0; 8_192], vec![0.0; 8_192]);
        // Two blocks: the first lets the wet mix settle.
        p.process(&mut m, &mut c);
        p.process(&mut m, &mut c);
        m
    };
    let dry = render(false);
    let wet = render(true);
    assert_ne!(dry, wet, "the crusher is audible");
    let diff = dry
        .iter()
        .zip(&wet)
        .fold(0.0f32, |m, (a, b)| m.max((a - b).abs()));
    assert!(diff > 0.01, "and not by a whisker: {diff}");
}

#[test]
fn the_echo_follows_the_deck_tempo_fader() {
    use engine::fx::FxKind;
    let (mut h, mut p) = channel(Engine::new(), 16);
    h.send(Command::Load(A, sine_track(440.0, 0.5))).unwrap();
    h.send(Command::SetBeatFrames(A, 24_000.0)).unwrap();
    h.send(Command::SetFxKind(A, FxKind::Echo)).unwrap();
    h.send(Command::SetRate(A, 2.0)).unwrap();
    let (mut m, mut c) = (vec![0.0; 512], vec![0.0; 512]);
    p.process(&mut m, &mut c);
    assert_eq!(
        p.fx_beat_frames(A),
        12_000.0,
        "a track played twice as fast has beats half as long"
    );
}

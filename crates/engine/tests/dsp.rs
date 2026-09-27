//! Channel DSP from spec 3.3: three-band isolator EQ with kills, one-knob filter, trim.
//! Frequency responses are measured by running sines through the processors.

use engine::dsp::{DjFilter, EqBand, Isolator, Trim};

const FS: f32 = 48_000.0;

/// Gain in dB of `process` for a stereo sine at `freq`, measured after settling.
fn gain_db(freq: f32, mut process: impl FnMut(&mut [f32])) -> f32 {
    let settle = (FS * 0.3) as usize;
    let measure = (FS * 0.2) as usize;
    let mut buf: Vec<f32> = (0..settle + measure)
        .flat_map(|i| {
            let s = (std::f32::consts::TAU * freq * i as f32 / FS).sin() * 0.5;
            [s, s]
        })
        .collect();
    let input_rms = 0.5 / 2f32.sqrt();
    for block in buf.chunks_mut(512) {
        process(block);
    }
    let tail = &buf[settle * 2..];
    let rms = (tail.iter().step_by(2).map(|s| s * s).sum::<f32>() / measure as f32).sqrt();
    20.0 * (rms / input_rms).max(1e-9).log10()
}

fn iso() -> Isolator {
    Isolator::new(FS)
}

// --- Isolator ---

#[test]
fn isolator_at_unity_is_flat() {
    for f in [40.0, 250.0, 800.0, 2500.0, 8000.0, 15000.0] {
        let mut eq = iso();
        let g = gain_db(f, |b| eq.process(b));
        assert!(g.abs() < 0.2, "{f} Hz: {g:.2} dB");
    }
}

#[test]
fn low_kill_removes_bass_and_keeps_highs() {
    let mut eq = iso();
    eq.set_kill(EqBand::Low, true);
    assert!(gain_db(50.0, |b| eq.process(b)) < -30.0);
    let mut eq = iso();
    eq.set_kill(EqBand::Low, true);
    assert!(gain_db(6000.0, |b| eq.process(b)).abs() < 0.5);
}

#[test]
fn high_kill_removes_treble_and_keeps_bass() {
    let mut eq = iso();
    eq.set_kill(EqBand::High, true);
    assert!(gain_db(12000.0, |b| eq.process(b)) < -30.0);
    let mut eq = iso();
    eq.set_kill(EqBand::High, true);
    assert!(gain_db(80.0, |b| eq.process(b)).abs() < 0.5);
}

#[test]
fn mid_kill_carves_out_the_middle() {
    let mut eq = iso();
    eq.set_kill(EqBand::Mid, true);
    assert!(gain_db(800.0, |b| eq.process(b)) < -25.0);
}

#[test]
fn band_gain_boosts_and_cuts_in_db() {
    let mut eq = iso();
    eq.set_gain_db(EqBand::Low, 6.0);
    let g = gain_db(40.0, |b| eq.process(b));
    assert!((g - 6.0).abs() < 0.3, "{g}");
    let mut eq = iso();
    eq.set_gain_db(EqBand::High, -12.0);
    let g = gain_db(12000.0, |b| eq.process(b));
    assert!((g + 12.0).abs() < 0.5, "{g}");
}

#[test]
fn gain_is_limited_to_plus_six_db() {
    let mut eq = iso();
    eq.set_gain_db(EqBand::Low, 40.0);
    assert!((gain_db(40.0, |b| eq.process(b)) - 6.0).abs() < 0.3);
}

#[test]
fn kills_are_smoothed_rather_than_instant() {
    let mut eq = iso();
    let mut sine = |start: usize, n: usize| -> Vec<f32> {
        (start..start + n).flat_map(|i| { let s = (std::f32::consts::TAU * 50.0 * i as f32 / FS).sin(); [s, s] }).collect()
    };
    let mut warm = sine(0, 24_000);
    eq.process(&mut warm);
    eq.set_kill(EqBand::Low, true);
    let mut first_ms = sine(24_000, 48);
    eq.process(&mut first_ms);
    let peak_first = first_ms.iter().fold(0f32, |m, s| m.max(s.abs()));
    assert!(peak_first > 0.3, "kill cut in instantly ({peak_first})");
    let mut later = sine(24_048, 9600);
    eq.process(&mut later);
    let peak_late = later[later.len() - 960..].iter().fold(0f32, |m, s| m.max(s.abs()));
    assert!(peak_late < 0.05, "kill never took effect ({peak_late})");
}

// --- One-knob filter ---

#[test]
fn filter_at_centre_is_an_exact_bypass() {
    let mut f = DjFilter::new(FS);
    let input: Vec<f32> = (0..1024).map(|i| ((i * 7919) % 1000) as f32 / 1000.0 - 0.5).collect();
    let mut buf = input.clone();
    f.process(&mut buf);
    assert_eq!(buf, input);
    f.set(0.02);
    f.process(&mut buf);
    assert_eq!(buf, input, "dead zone should also bypass");
}

#[test]
fn full_left_is_a_low_pass() {
    let mut f = DjFilter::new(FS);
    f.set(-1.0);
    assert!(gain_db(5000.0, |b| f.process(b)) < -30.0);
    let mut f = DjFilter::new(FS);
    f.set(-1.0);
    assert!(gain_db(25.0, |b| f.process(b)).abs() < 1.5);
}

#[test]
fn full_right_is_a_high_pass() {
    let mut f = DjFilter::new(FS);
    f.set(1.0);
    assert!(gain_db(200.0, |b| f.process(b)) < -30.0);
    let mut f = DjFilter::new(FS);
    f.set(1.0);
    assert!(gain_db(18000.0, |b| f.process(b)).abs() < 1.5);
}

#[test]
fn turning_further_filters_more() {
    let g = |v: f32| {
        let mut f = DjFilter::new(FS);
        f.set(v);
        gain_db(3000.0, |b| f.process(b))
    };
    assert!(g(-0.8) < g(-0.4) - 6.0);
    assert!(g(-0.4) < g(-0.1));
}

#[test]
fn sweeping_the_filter_stays_stable() {
    let mut f = DjFilter::new(FS);
    let mut peak = 0f32;
    for block in 0..400 {
        f.set(((block as f32) * 0.05).sin());
        let mut buf: Vec<f32> = (0..256).map(|i| (((block * 256 + i) * 2654435761usize) % 2000) as f32 / 1000.0 - 1.0).collect();
        f.process(&mut buf);
        peak = buf.iter().fold(peak, |m, s| if s.is_finite() { m.max(s.abs()) } else { f32::INFINITY });
    }
    assert!(peak < 4.0, "filter blew up: {peak}");
}

// --- Trim ---

#[test]
fn trim_applies_gain_in_db_within_plus_minus_twelve() {
    let mut t = Trim::new(FS);
    t.set_db(6.0);
    assert!((gain_db(1000.0, |b| t.process(b)) - 6.0).abs() < 0.1);
    let mut t = Trim::new(FS);
    t.set_db(-30.0);
    assert!((gain_db(1000.0, |b| t.process(b)) + 12.0).abs() < 0.1);
}

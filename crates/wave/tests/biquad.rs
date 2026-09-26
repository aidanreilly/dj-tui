use wave::Biquad;

const SR: f32 = 48_000.0;

/// RMS of `secs` seconds of a sine at `hz` after passing through `f`.
fn rms(mut f: Biquad, hz: f32, secs: f32) -> f32 {
    let n = (SR * secs) as usize;
    let mut sum = 0.0f64;
    for i in 0..n {
        let x = (std::f32::consts::TAU * hz * i as f32 / SR).sin();
        let y = f.process(x);
        // Skip the filter's settling time so the figure describes steady state.
        if i > n / 4 {
            sum += (y * y) as f64;
        }
    }
    (sum / (n - n / 4) as f64).sqrt() as f32
}

const UNIT: f32 = std::f32::consts::FRAC_1_SQRT_2; // RMS of a unit sine

#[test]
fn low_pass_keeps_bass_and_rejects_treble() {
    assert!((rms(Biquad::low_pass(250.0, SR), 60.0, 0.5) - UNIT).abs() < 0.05);
    assert!(rms(Biquad::low_pass(250.0, SR), 8_000.0, 0.5) < 0.01);
}

#[test]
fn high_pass_keeps_treble_and_rejects_bass() {
    assert!((rms(Biquad::high_pass(2_500.0, SR), 8_000.0, 0.5) - UNIT).abs() < 0.05);
    assert!(rms(Biquad::high_pass(2_500.0, SR), 60.0, 0.5) < 0.01);
}

#[test]
fn cutoff_at_the_corner_is_about_minus_three_decibels() {
    let at_corner = rms(Biquad::low_pass(250.0, SR), 250.0, 0.5);
    assert!((at_corner / UNIT - 0.707).abs() < 0.05, "got {at_corner}");
}

#[test]
fn a_cutoff_above_nyquist_is_clamped_rather_than_producing_nonsense() {
    // An 8 kHz file cannot carry a 2.5 kHz high-pass corner at Q 0.707 without
    // exceeding Nyquist; the filter must stay finite.
    let mut f = Biquad::high_pass(2_500.0, 4_000.0);
    for i in 0..1000 {
        let y = f.process((i % 7) as f32 * 0.1);
        assert!(y.is_finite(), "sample {i} was {y}");
    }
}

#[test]
fn reset_clears_history() {
    let mut f = Biquad::low_pass(250.0, SR);
    for _ in 0..1000 {
        f.process(1.0);
    }
    f.reset();
    assert_eq!(f.process(0.0), 0.0);
}

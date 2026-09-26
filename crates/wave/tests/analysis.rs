use wave::{analyse, WavePoint, POINTS_PER_SECOND};

const SR: u32 = 48_000;

/// `secs` of a sine at `hz`, the same in both channels.
fn sine(hz: f32, secs: f32) -> (Vec<f32>, Vec<f32>) {
    let n = (SR as f32 * secs) as usize;
    let v: Vec<f32> = (0..n)
        .map(|i| (std::f32::consts::TAU * hz * i as f32 / SR as f32).sin())
        .collect();
    (v.clone(), v)
}

/// Index of the largest band in a point.
fn loudest(p: &WavePoint) -> usize {
    let mut best = 0;
    for i in 1..3 {
        if p.bands[i] > p.bands[best] {
            best = i;
        }
    }
    best
}

/// Average of a band over every point, skipping the first to avoid filter settling.
fn mean(points: &[WavePoint], band: usize) -> f32 {
    let tail = &points[1..];
    tail.iter().map(|p| p.bands[band]).sum::<f32>() / tail.len() as f32
}

#[test]
fn a_sixty_hertz_sine_lands_in_the_low_band() {
    let (l, r) = sine(60.0, 2.0);
    let points = analyse(&l, &r, SR);
    assert_eq!(loudest(&points[points.len() / 2]), 0);
    assert!(
        mean(&points, 2) < 0.05,
        "high band was {}",
        mean(&points, 2)
    );
}

#[test]
fn a_one_kilohertz_sine_lands_in_the_mid_band() {
    let (l, r) = sine(1_000.0, 2.0);
    let points = analyse(&l, &r, SR);
    assert_eq!(loudest(&points[points.len() / 2]), 1);
}

#[test]
fn an_eight_kilohertz_sine_lands_in_the_high_band() {
    let (l, r) = sine(8_000.0, 2.0);
    let points = analyse(&l, &r, SR);
    assert_eq!(loudest(&points[points.len() / 2]), 2);
    assert!(mean(&points, 0) < 0.05, "low band was {}", mean(&points, 0));
}

#[test]
fn point_count_follows_duration_at_twenty_per_second() {
    let (l, r) = sine(440.0, 3.0);
    assert_eq!(analyse(&l, &r, SR).len(), 3 * POINTS_PER_SECOND as usize);
}

#[test]
fn every_value_is_normalised_into_zero_to_one() {
    let (l, r) = sine(440.0, 1.0);
    for p in analyse(&l, &r, SR) {
        assert!(p.range[0] >= -1.0 && p.range[0] <= 0.0, "min {:?}", p.range);
        assert!(p.range[1] >= 0.0 && p.range[1] <= 1.0, "max {:?}", p.range);
        for b in p.bands {
            assert!((0.0..=1.0).contains(&b), "band {b}");
        }
    }
}

#[test]
fn the_loudest_sample_normalises_to_one() {
    let (l, r) = sine(440.0, 1.0);
    let points = analyse(&l, &r, SR);
    let peak = points
        .iter()
        .flat_map(|p| [p.range[0].abs(), p.range[1].abs()])
        .fold(0.0f32, f32::max);
    assert!((peak - 1.0).abs() < 1e-6, "peak was {peak}");
}

#[test]
fn silence_is_all_zeros_without_dividing_by_zero() {
    let l = vec![0.0f32; SR as usize];
    let points = analyse(&l, &l, SR);
    assert_eq!(points.len(), POINTS_PER_SECOND as usize);
    for p in points {
        assert_eq!(p, WavePoint::default());
    }
}

#[test]
fn audio_shorter_than_one_bucket_still_gives_one_point() {
    // 2 ms at 48 kHz is 96 frames; 96 * 20 / 48000 rounds down to zero buckets.
    let (l, r) = sine(440.0, 0.002);
    let points = analyse(&l, &r, SR);
    assert_eq!(points.len(), 1);
    for b in points[0].bands {
        assert!(b.is_finite(), "band was {b}");
    }
}

#[test]
fn no_audio_gives_no_points() {
    assert!(analyse(&[], &[], SR).is_empty());
}

#[test]
fn mismatched_channel_lengths_use_the_shorter_one() {
    let (l, _) = sine(440.0, 2.0);
    let r = l[..l.len() / 2].to_vec();
    assert_eq!(analyse(&l, &r, SR).len(), POINTS_PER_SECOND as usize);
}

#[test]
fn a_steady_tone_does_not_spike_at_bucket_boundaries() {
    // Filter state carries across buckets, so consecutive points of a steady tone
    // must sit close together.
    let (l, r) = sine(1_000.0, 2.0);
    let points = analyse(&l, &r, SR);
    for w in points[2..].windows(2) {
        let step = (w[1].bands[1] - w[0].bands[1]).abs();
        assert!(step < 0.02, "mid band jumped by {step}");
    }
}

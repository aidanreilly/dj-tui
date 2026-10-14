//! Bars, phrase lines and sections, read from the band envelope the overview already keeps.
//!
//! Dance music is built in blocks of four, eight and sixteen bars, and what changes at a
//! block line is mostly the kick. So the low band, averaged per bar, is enough to tell a
//! stretch with the kick in from one without. That is what a DJ looks at on a waveform to
//! find the intro, the breakdown and the outro, and it is all the transitions need.
//!
//! The beat grid knows where a beat falls but not which beat starts the bar. The bar line is
//! taken as whichever of the four beat positions the low band changes on most often.

use crate::tempo::BeatGrid;

/// What a stretch of a track is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKind {
    /// Before the kick first comes in.
    Intro,
    /// Kick in, the first time round.
    Body,
    /// The kick drops out for a while between two stretches with it.
    Breakdown,
    /// Kick back in after a breakdown.
    Drop,
    /// After the kick leaves for good.
    Outro,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Section {
    pub kind: SectionKind,
    pub start_bar: usize,
    pub bars: usize,
}

impl Section {
    pub fn end_bar(&self) -> usize {
        self.start_bar + self.bars
    }

    /// The kick is in.
    pub fn is_full(&self) -> bool {
        matches!(self.kind, SectionKind::Body | SectionKind::Drop)
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Phrases {
    /// Which beat of the grid, 0 to 3, the first bar starts on.
    pub downbeat_beat: usize,
    /// Mean low, mid and high level of each bar, each band scaled to its loudest bars.
    pub bar_levels: Vec<[f32; 3]>,
    pub sections: Vec<Section>,
}

/// A block is four bars: the shortest unit a section is built from.
const BLOCK_BARS: usize = 4;
/// A gap in the kick shorter than this is a fill, not a breakdown.
const MIN_BREAKDOWN_BARS: usize = 8;
/// A bar has the kick in when its low band reaches this share of the track's loud bars.
const FULL_SHARE: f32 = 0.5;
/// And this much in absolute terms, so a track with no kick at all is not all kick.
const FULL_FLOOR: f32 = 0.08;

impl Phrases {
    pub fn bar_count(&self) -> usize {
        self.bar_levels.len()
    }

    /// Where bar `bar` starts, in seconds of the track.
    pub fn bar_secs(&self, grid: &BeatGrid, bar: usize) -> f64 {
        grid.first_beat_secs + (self.downbeat_beat + bar * 4) as f64 * grid.beat_secs()
    }

    /// Which bar `secs` falls in, with the fraction of the way through it.
    pub fn bar_at(&self, grid: &BeatGrid, secs: f64) -> f64 {
        ((secs - grid.first_beat_secs) / grid.beat_secs() - self.downbeat_beat as f64) / 4.0
    }

    pub fn section_at(&self, bar: usize) -> Option<&Section> {
        self.sections
            .iter()
            .find(|s| (s.start_bar..s.end_bar()).contains(&bar))
    }

    /// The bar to bring a track in on when it should land at full energy: the first drop
    /// after a breakdown, or failing that where the kick first comes in.
    pub fn first_drop(&self) -> Option<usize> {
        let first = |kind| self.sections.iter().find(|s| s.kind == kind);
        first(SectionKind::Drop)
            .or_else(|| first(SectionKind::Body).filter(|s| s.start_bar > 0))
            .map(|s| s.start_bar)
    }

    /// Where the kick goes for good, if it does before the end.
    pub fn outro_start(&self) -> Option<usize> {
        self.sections
            .iter()
            .find(|s| s.kind == SectionKind::Outro)
            .map(|s| s.start_bar)
    }

    /// Where the kick first comes in, if the track has an intro before it.
    pub fn intro_end(&self) -> Option<usize> {
        self.sections
            .first()
            .filter(|s| s.kind == SectionKind::Intro)
            .map(Section::end_bar)
    }

    /// Bars where one section hands over to the next.
    pub fn boundaries(&self) -> impl Iterator<Item = usize> + '_ {
        self.sections.iter().skip(1).map(|s| s.start_bar)
    }
}

/// Read bars and sections from `bands`, the overview's per-band envelope spread evenly over
/// `duration_secs`. An empty envelope or a zero duration gives an empty result.
pub fn detect_phrases(bands: &[[f32; 3]], duration_secs: f64, grid: &BeatGrid) -> Phrases {
    if bands.is_empty() || duration_secs <= 0.0 || grid.bpm <= 0.0 {
        return Phrases::default();
    }
    let points_per_sec = bands.len() as f64 / duration_secs;
    let beat = grid.beat_secs();
    let level = |from: f64, to: f64| -> [f32; 3] {
        let a = ((from * points_per_sec).round().max(0.0) as usize).min(bands.len());
        let b = ((to * points_per_sec).round().max(0.0) as usize).min(bands.len());
        if b <= a {
            return [0.0; 3];
        }
        let mut sum = [0.0f32; 3];
        for p in &bands[a..b] {
            for (s, v) in sum.iter_mut().zip(p) {
                *s += v;
            }
        }
        sum.map(|s| s / (b - a) as f32)
    };

    // The low band per beat, for finding the bar line.
    let beats = ((duration_secs - grid.first_beat_secs) / beat)
        .floor()
        .max(0.0) as usize;
    let lows: Vec<f32> = (0..beats)
        .map(|k| {
            let t = grid.first_beat_secs + k as f64 * beat;
            level(t, t + beat)[0]
        })
        .collect();
    let downbeat_beat = downbeat(&lows);

    let bar_secs = beat * 4.0;
    let start = grid.first_beat_secs + downbeat_beat as f64 * beat;
    let bars = ((duration_secs - start) / bar_secs).floor().max(0.0) as usize;
    let raw: Vec<[f32; 3]> = (0..bars)
        .map(|n| {
            let t = start + n as f64 * bar_secs;
            level(t, t + bar_secs)
        })
        .collect();
    let raw_loud = percentile(raw.iter().map(|l| l[0]), 0.9);
    let full: Vec<bool> = raw
        .iter()
        .map(|l| l[0] >= FULL_FLOOR && l[0] >= raw_loud * FULL_SHARE)
        .collect();
    // Scale each band to its loud bars, so a quiet master and a loud one read the same.
    let mut bar_levels = raw;
    for band in 0..3 {
        let loud = percentile(bar_levels.iter().map(|l| l[band]), 0.9);
        if loud > 0.0 {
            bar_levels.iter_mut().for_each(|l| l[band] /= loud);
        }
    }

    Phrases {
        downbeat_beat,
        sections: sections(&full),
        bar_levels,
    }
}

/// Which of the four beat positions the low band changes on most.
fn downbeat(lows: &[f32]) -> usize {
    if lows.len() < 8 {
        return 0;
    }
    let mut score = [0.0f32; 4];
    for b in 4..lows.len().saturating_sub(4) {
        let before: f32 = lows[b - 4..b].iter().sum();
        let after: f32 = lows[b..b + 4].iter().sum();
        // Squared, so the beat a change lands on outweighs its neighbours. Summed plainly,
        // the four positions around one step score exactly the same.
        score[b % 4] += (after - before).powi(2);
    }
    // Ties go to the earliest, so a track with no changes keeps the grid's own first beat.
    let mut best = 0;
    for (i, s) in score.iter().enumerate() {
        if *s > score[best] * 1.0001 {
            best = i;
        }
    }
    best
}

fn percentile(values: impl Iterator<Item = f32>, q: f32) -> f32 {
    let mut v: Vec<f32> = values.collect();
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    v[((v.len() - 1) as f32 * q).round() as usize]
}

/// Runs of kick and no kick, decided four bars at a time, then named.
fn sections(full: &[bool]) -> Vec<Section> {
    if full.is_empty() {
        return Vec::new();
    }
    // A block counts as empty only when three of its four bars are, so a fill does not
    // split a section.
    let block_full: Vec<bool> = full
        .chunks(BLOCK_BARS)
        .map(|c| c.iter().filter(|&&f| !f).count() * 4 < c.len() * 3)
        .collect();
    let mut runs: Vec<(bool, usize, usize)> = Vec::new();
    for (i, &f) in block_full.iter().enumerate() {
        let bar = i * BLOCK_BARS;
        match runs.last_mut() {
            Some((kind, _, len)) if *kind == f => *len += BLOCK_BARS,
            _ => runs.push((f, bar, BLOCK_BARS)),
        }
    }
    // The last block may be short.
    if let Some(last) = runs.last_mut() {
        last.2 = full.len() - last.1;
    }
    // A short gap between two runs with the kick is a fill: fold it into them.
    let mut merged: Vec<(bool, usize, usize)> = Vec::new();
    for (i, run) in runs.iter().enumerate() {
        let inner = i > 0 && i + 1 < runs.len();
        let fill = !run.0 && inner && run.2 < MIN_BREAKDOWN_BARS;
        let as_full = (run.0 || fill, run.1, run.2);
        match merged.last_mut() {
            Some(prev) if prev.0 == as_full.0 => prev.2 += as_full.2,
            _ => merged.push(as_full),
        }
    }
    if merged.iter().all(|r| !r.0) {
        return vec![Section {
            kind: SectionKind::Body,
            start_bar: 0,
            bars: full.len(),
        }];
    }
    let first_full = merged.iter().position(|r| r.0).unwrap_or(0);
    let last_full = merged.iter().rposition(|r| r.0).unwrap_or(0);
    merged
        .iter()
        .enumerate()
        .map(|(i, &(f, start_bar, bars))| {
            let kind = match (f, i) {
                (false, i) if i < first_full => SectionKind::Intro,
                (false, i) if i > last_full => SectionKind::Outro,
                (false, _) => SectionKind::Breakdown,
                (true, i) if i == first_full => SectionKind::Body,
                (true, _) => SectionKind::Drop,
            };
            Section {
                kind,
                start_bar,
                bars,
            }
        })
        .collect()
}

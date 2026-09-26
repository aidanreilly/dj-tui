//! Pixel-resolution overview waveform for terminals with a bitmap protocol.
//!
//! Pure image code: signed peak ranges in, RGBA images out. [`PixelWaveform`] decides when
//! a new image has to go to the terminal, so a playing deck only sends one when the
//! playhead reaches a new pixel column.
//!
//! The anti-aliased span drawing follows `draw_vspan_aa` in tui-wave
//! (<https://github.com/biomassa/tui-wave>, MIT, Copyright (c) 2026 biomassa).

use crate::waveform::downsample_points;
use wave::WavePoint;
use image::{Rgba, RgbaImage};
use std::hash::{Hash, Hasher};

/// Width of the playhead line in pixels.
pub const PLAYHEAD_WIDTH: u32 = 2;

/// How bright the played portion is relative to the rest.
const DIM: f32 = 0.4;

/// Same contrast curve as the glyph renderer, so both modes read alike.
fn shape(peak: f32) -> f32 {
    peak.clamp(0.0, 1.0).powf(1.5)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Palette {
    /// Colour at the centre line.
    pub low: Rgba<u8>,
    /// Colour at full scale.
    pub high: Rgba<u8>,
    pub centre_line: Rgba<u8>,
    pub playhead: Rgba<u8>,
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            low: Rgba([96, 84, 170, 255]),
            high: Rgba([214, 208, 255, 255]),
            centre_line: Rgba([58, 50, 84, 255]),
            playhead: Rgba([236, 232, 210, 255]),
        }
    }
}

fn lerp(a: Rgba<u8>, b: Rgba<u8>, t: f32) -> Rgba<u8> {
    let t = t.clamp(0.0, 1.0);
    let m = |i: usize| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round() as u8;
    Rgba([m(0), m(1), m(2), 255])
}

/// Source-over blend of `color` at `coverage` onto the pixel, which may be transparent.
fn blend(img: &mut RgbaImage, x: u32, y: u32, color: Rgba<u8>, coverage: f32) {
    let src_a = coverage.clamp(0.0, 1.0) * (color[3] as f32 / 255.0);
    if src_a <= 0.0 {
        return;
    }
    let dst = *img.get_pixel(x, y);
    let dst_a = dst[3] as f32 / 255.0;
    let out_a = src_a + dst_a * (1.0 - src_a);
    let ch = |i: usize| {
        ((color[i] as f32 * src_a + dst[i] as f32 * dst_a * (1.0 - src_a)) / out_a).round() as u8
    };
    img.put_pixel(
        x,
        y,
        Rgba([ch(0), ch(1), ch(2), (out_a * 255.0).round() as u8]),
    );
}

/// Fill pixel column `x` over the continuous range `[lo, hi]`, blending partial rows.
fn span(img: &mut RgbaImage, x: u32, lo: f32, hi: f32, color_at: impl Fn(u32) -> Rgba<u8>) {
    let h = img.height() as f32;
    let (lo, hi) = (lo.max(0.0), hi.min(h));
    if hi <= lo {
        return;
    }
    let first = lo.floor() as u32;
    let last = (hi.ceil() as u32).min(img.height());
    for y in first..last {
        let coverage = hi.min(y as f32 + 1.0) - lo.max(y as f32);
        blend(img, x, y, color_at(y), coverage);
    }
}

/// A deck's waveform drawn once, in normal and dimmed (already played) versions.
#[derive(Debug, Clone)]
pub struct WaveformBitmaps {
    normal: RgbaImage,
    dimmed: RgbaImage,
}

impl WaveformBitmaps {
    pub fn rasterize(points: &[WavePoint], width: u32, height: u32, palette: &Palette) -> Self {
        let mut normal = RgbaImage::new(width, height);
        let mid = height as f32 / 2.0;
        let columns = downsample_points(points, width as usize);
        for (x, p) in columns.iter().enumerate() {
            let [min, max] = p.range;
            let x = x as u32;
            let half = shape(min.abs().max(max.abs())) * mid;
            if half <= 0.0 {
                continue;
            }
            // Keep very quiet passages visible: at least one pixel, centred.
            let half = half.max(0.5);
            span(&mut normal, x, mid - half, mid + half, |y| {
                let t = ((y as f32 + 0.5) - mid).abs() / mid;
                lerp(palette.low, palette.high, t)
            });
        }
        // The centre line fills whatever the bars left empty, straddling the middle.
        for x in 0..width {
            // saturating: a zero-height area during a resize leaves `mid` at 0.
            for y in [(mid.ceil() as u32).saturating_sub(1), mid as u32] {
                if y < height && normal.get_pixel(x, y)[3] == 0 {
                    normal.put_pixel(x, y, palette.centre_line);
                }
            }
        }
        let mut dimmed = normal.clone();
        for p in dimmed.pixels_mut() {
            for c in 0..3 {
                p[c] = (p[c] as f32 * DIM).round() as u8;
            }
        }
        Self { normal, dimmed }
    }

    pub fn normal(&self) -> &RgbaImage {
        &self.normal
    }

    pub fn dimmed(&self) -> &RgbaImage {
        &self.dimmed
    }

    /// The image to show: played part dimmed, then the playhead line.
    pub fn compose(&self, playhead: Option<u32>, palette: &Palette) -> RgbaImage {
        let mut img = self.normal.clone();
        let Some(px) = playhead else { return img };
        let (w, h) = img.dimensions();
        for y in 0..h {
            for x in 0..px.min(w) {
                img.put_pixel(x, y, *self.dimmed.get_pixel(x, y));
            }
            for x in px.min(w)..(px + PLAYHEAD_WIDTH).min(w) {
                img.put_pixel(x, y, palette.playhead);
            }
        }
        img
    }
}

/// Pixel column of the playhead's left edge, or `None` without a track.
pub fn playhead_x(position_secs: f64, duration_secs: f64, width: u32) -> Option<u32> {
    if duration_secs <= 0.0 || width == 0 {
        return None;
    }
    let frac = (position_secs / duration_secs).clamp(0.0, 1.0);
    Some(((frac * width as f64) as u32).min(width.saturating_sub(PLAYHEAD_WIDTH)))
}

fn fingerprint(points: &[WavePoint]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    points.len().hash(&mut h);
    for p in points {
        for v in [p.range[0], p.range[1], p.bands[0], p.bands[1], p.bands[2]] {
            v.to_bits().hash(&mut h);
        }
    }
    h.finish()
}

/// Per-deck cache. Rasterises on a new track or size, recomposes when the playhead
/// crosses into a new pixel column, and otherwise reports that nothing changed.
#[derive(Default)]
pub struct PixelWaveform {
    source: Option<(u64, (u32, u32))>,
    bitmaps: Option<WaveformBitmaps>,
    shown_playhead: Option<Option<u32>>,
    rasterizations: u64,
}

impl PixelWaveform {
    /// Returns an image when the terminal needs a new one.
    pub fn update(
        &mut self,
        points: &[WavePoint],
        size: (u32, u32),
        playhead: Option<u32>,
        palette: &Palette,
    ) -> Option<RgbaImage> {
        if points.is_empty() || size.0 == 0 || size.1 == 0 {
            *self = Self {
                rasterizations: self.rasterizations,
                ..Default::default()
            };
            return None;
        }
        let source = (fingerprint(points), size);
        if self.source != Some(source) || self.bitmaps.is_none() {
            self.bitmaps = Some(WaveformBitmaps::rasterize(points, size.0, size.1, palette));
            self.source = Some(source);
            self.shown_playhead = None;
            self.rasterizations += 1;
        }
        if self.shown_playhead == Some(playhead) {
            return None;
        }
        self.shown_playhead = Some(playhead);
        self.bitmaps.as_ref().map(|b| b.compose(playhead, palette))
    }

    pub fn has_image(&self) -> bool {
        self.bitmaps.is_some()
    }

    pub fn rasterizations(&self) -> u64 {
        self.rasterizations
    }
}

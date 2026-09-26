//! Pixel-resolution overview waveform for terminals with a bitmap protocol.
//!
//! Pure image code: signed peak ranges in, RGBA images out. [`PixelWaveform`] decides when
//! a new image has to go to the terminal, so a playing deck only sends one when the
//! playhead reaches a new pixel column.
//!
//! The anti-aliased span drawing follows `draw_vspan_aa` in tui-wave
//! (<https://github.com/biomassa/tui-wave>, MIT, Copyright (c) 2026 biomassa).

use crate::waveform::downsample_points;
use image::{Rgba, RgbaImage};
use std::hash::{Hash, Hasher};
use wave::{WavePoint, WaveformMode};

/// Width of the playhead line in pixels.
pub const PLAYHEAD_WIDTH: u32 = 2;

/// How bright the played portion is relative to the rest.
const DIM: f32 = 0.4;

/// Same contrast curve as the glyph renderer, so both modes read alike.
fn shape(peak: f32) -> f32 {
    peak.clamp(0.0, 1.0).powf(1.5)
}

/// Per-band display gain, applied at draw time and never stored in a cache.
///
/// The three bands share one divisor, and music puts far more absolute energy into bass
/// than into cymbals, so an ungained high band would sit near 0.1 and all but vanish
/// once `shape` raises it to the power of 1.5.
pub const BAND_GAIN: [f32; 3] = [1.0, 1.3, 1.6];

/// How far `rgb` measures each band against its own loudest column rather than against
/// the track's overall peak. At 0 a bass-led track is red end to end; at 1 every band is
/// stretched to its own full range and almost every column lands mid-ramp. In between
/// keeps the spectral tilt while still reaching the blue end on treble-led passages.
const RGB_LEVELLING: f32 = 0.5;

/// Colour at the top of the `blue` mode's tint.
const WHITE: Rgba<u8> = Rgba([240, 244, 255, 255]);

#[derive(Debug, Clone, PartialEq)]
pub struct Palette {
    /// Low, mid and high colours. In `rgb` these are the pure channels.
    pub bands: [Rgba<u8>; 3],
    pub centre_line: Rgba<u8>,
    pub playhead: Rgba<u8>,
    pub mode: WaveformMode,
}

impl Palette {
    pub fn for_mode(mode: WaveformMode) -> Self {
        let bands = match mode {
            WaveformMode::ThreeBand | WaveformMode::Blue => [
                Rgba([36, 82, 200, 255]),
                Rgba([235, 150, 40, 255]),
                Rgba([238, 240, 248, 255]),
            ],
            WaveformMode::Rgb => [
                Rgba([255, 0, 0, 255]),
                Rgba([0, 255, 0, 255]),
                Rgba([0, 0, 255, 255]),
            ],
        };
        Self {
            bands,
            centre_line: Rgba([58, 50, 84, 255]),
            playhead: Rgba([236, 232, 210, 255]),
            mode,
        }
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self::for_mode(WaveformMode::default())
    }
}

/// Fully saturated colour for a spectral position: 0 is red, 0.5 green, 1 blue,
/// passing through yellow and cyan. Used by `rgb`, whose hue carries the frequency
/// balance while height carries the level.
fn spectrum(position: f32) -> Rgba<u8> {
    let h = position.clamp(0.0, 1.0) * 4.0;
    let i = h.floor();
    let f = h - i;
    let (r, g, b) = match i as u32 {
        0 => (1.0, f, 0.0),
        1 => (1.0 - f, 1.0, 0.0),
        2 => (0.0, 1.0, f),
        3 => (0.0, 1.0 - f, 1.0),
        _ => (0.0, 0.0, 1.0),
    };
    Rgba([
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
        255,
    ])
}

/// A band's height as a fraction of the half panel, with its gain and the contrast
/// curve applied. Shared with the glyph renderer so both modes read alike.
pub fn band_half_steps(value: f32, band: usize) -> f32 {
    shape((value * BAND_GAIN[band]).min(1.0))
}

fn band_half(value: f32, band: usize, mid: f32) -> f32 {
    band_half_steps(value, band) * mid
}

/// Paint one column, in whichever scheme the palette names.
fn draw_column(
    img: &mut RgbaImage,
    x: u32,
    mid: f32,
    p: &WavePoint,
    palette: &Palette,
    reference: [f32; 3],
) {
    match palette.mode {
        WaveformMode::ThreeBand => {
            let mut drew = false;
            // Low first, then mid, then high, so the highest band present wins where
            // they overlap. Bass reaches furthest, which leaves a white core at the
            // centre line.
            for band in 0..3 {
                let half = band_half(p.bands[band], band, mid);
                if half <= 0.0 {
                    continue;
                }
                drew = true;
                let colour = palette.bands[band];
                let half = half.max(0.5);
                span(img, x, mid - half, mid + half, |_| colour);
            }
            if !drew {
                // Every band at zero with a real range: draw the shape in the low colour
                // rather than dropping the column.
                let half = shape(p.range[0].abs().max(p.range[1].abs())) * mid;
                if half > 0.0 {
                    let colour = palette.bands[0];
                    span(img, x, mid - half.max(0.5), mid + half.max(0.5), |_| colour);
                }
            }
        }
        WaveformMode::Rgb => {
            // One colour per column: the three bands summed onto their own channels,
            // so a full-spectrum column trends toward white.
            let half = shape(p.range[0].abs().max(p.range[1].abs())) * mid;
            if half <= 0.0 {
                return;
            }
            // Hue follows where the energy sits, which is what makes a whole track
            // readable: red for a bass-led column, green through the mids, blue for a
            // treble-led one. Squaring turns amplitude into energy so the leading band
            // pulls the hue decisively instead of every column averaging to grey.
            let mut energy = [0.0f32; 3];
            for (e, (v, r)) in energy.iter_mut().zip(p.bands.iter().zip(reference)) {
                let scaled = if r > 0.0 {
                    v / r.powf(RGB_LEVELLING)
                } else {
                    0.0
                };
                *e = scaled * scaled;
            }
            let total: f32 = energy.iter().sum();
            let colour = if total <= 0.0 {
                palette.bands[0]
            } else {
                spectrum((0.5 * energy[1] + energy[2]) / total)
            };
            let half = half.max(0.5);
            span(img, x, mid - half, mid + half, |_| colour);
        }
        WaveformMode::Blue => {
            // Height from the three bands combined, tinting toward white as the high
            // band's share rises.
            let power: f32 = p.bands.iter().map(|b| b * b).sum();
            let half = shape(power.sqrt().min(1.0)) * mid;
            if half <= 0.0 {
                return;
            }
            let tint = (p.bands[2] * BAND_GAIN[2]).min(1.0);
            let colour = lerp(palette.bands[0], WHITE, tint);
            let half = half.max(0.5);
            span(img, x, mid - half, mid + half, |_| colour);
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
        // Each band's own loudest column. `rgb` measures a band against this rather than
        // against the track's overall peak, so a hi-hat section reads as treble-led even
        // though hats never approach a kick in absolute level.
        let mut reference = [0.0f32; 3];
        for p in &columns {
            for (r, v) in reference.iter_mut().zip(p.bands) {
                *r = r.max(v);
            }
        }
        for (x, p) in columns.iter().enumerate() {
            draw_column(&mut normal, x as u32, mid, p, palette, reference);
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
    source: Option<(u64, (u32, u32), WaveformMode)>,
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
        let source = (fingerprint(points), size, palette.mode);
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

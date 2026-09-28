//! Renders full dj-tui screens for documentation, without a real terminal.
//!     cargo run --release --example screenshots -- /tmp/shots
//! For each waveform mode it writes the text screen as a cell dump (`<mode>.cells`, one
//! tab-separated line per cell) plus the kitty-graphics waveform images the Graphics layer
//! would send (`<mode>_deck<N>.rgba`, raw RGBA with a `w h` header line).
//! `scripts/screenshots.py` turns these into PNGs.

use engine::fx::FxKind;
use engine::{DeckId, Track};
use loader::{band_envelope, waveform_envelope, ENVELOPE_POINTS};
use ratatui::{backend::TestBackend, layout::Rect, Terminal};
use std::f32::consts::TAU;
use std::io::Write;
use tui::pixel::{loop_columns, playhead_x, Palette, Wave, WaveformBitmaps, WaveformMode};
use tui::{
    render_screen, screen_layout, waveform_area, DeckView, MixerView, ScreenView, StripView,
};

const FS: u32 = 48_000;
const COLS: u16 = 120;
const ROWS: u16 = 44;
/// Cell size assumed for the pixel images; typical for Ghostty at 13 pt.
const CELL: (u32, u32) = (10, 20);

/// 64 bars at 128 BPM: drums intro, full drop, mid-only breakdown, drop, hats outro.
fn synth(offset: usize) -> Track {
    let beat = 60.0 / 128.0;
    let secs = beat * 4.0 * 64.0;
    let n = (secs * FS as f32) as usize;
    let mut rng = 0x1234_5678u32;
    let mut noise = move || {
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        rng as f32 / u32::MAX as f32 * 2.0 - 1.0
    };
    let mut hp = 0.0f32;
    let mut prev = 0.0f32;
    let mut data = Vec::with_capacity(n * 2);
    for i in 0..n {
        let t = i as f32 / FS as f32;
        let bar = ((t / (beat * 4.0)) as usize + offset) % 64;
        let tb = t % beat;
        let (kick_on, bass_on, vox_on, hat_on) = match bar {
            0..=7 => (true, false, false, true),
            8..=23 => (true, true, true, true),
            24..=35 => (false, false, true, false),
            36..=55 => (true, true, true, true),
            _ => (false, false, false, true),
        };
        let kick = if kick_on {
            (-tb * 18.0).exp() * (TAU * (50.0 + 90.0 * (-tb * 30.0).exp()) * tb).sin()
        } else {
            0.0
        };
        let bass = if bass_on && tb > beat * 0.5 {
            0.45 * (TAU * 55.0 * t).sin()
        } else {
            0.0
        };
        let vox = if vox_on {
            0.3 * (TAU * 700.0 * t + 3.0 * (TAU * 5.0 * t).sin()).sin()
                * (0.6 + 0.4 * (TAU * 0.25 * t).sin())
        } else {
            0.0
        };
        let x = noise();
        hp = 0.2 * (hp + x - prev);
        prev = x;
        let hat = if hat_on && (tb - beat * 0.5).abs() < 0.03 {
            2.5 * hp
        } else {
            0.0
        };
        let s = (0.9 * kick + bass + vox + hat).clamp(-1.0, 1.0);
        data.extend_from_slice(&[s, s]);
    }
    Track::from_interleaved(data, FS)
}

struct Deck {
    ranges: Vec<[f32; 2]>,
    bands: Vec<[f32; 3]>,
    secs: f64,
}

fn deck(offset: usize) -> Deck {
    let t = synth(offset);
    Deck {
        ranges: waveform_envelope(&t, ENVELOPE_POINTS),
        bands: band_envelope(&t, ENVELOPE_POINTS),
        secs: t.frames() as f64 / FS as f64,
    }
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let decks = [deck(0), deck(20)];
    let titles = ["Synthetic - Warehouse Tool", "Synthetic - Breakdown Edit"];
    let positions = [41.0, 101.0];
    for (mode, name) in [
        (WaveformMode::ThreeBand, "3band"),
        (WaveformMode::Rgb, "rgb"),
        (WaveformMode::Blue, "blue"),
    ] {
        let hot_cue_secs: [[Option<f64>; 8]; 2] = [
            [
                Some(15.0),
                Some(45.0),
                None,
                Some(75.0),
                None,
                None,
                None,
                None,
            ],
            [
                Some(7.5),
                None,
                Some(52.5),
                None,
                None,
                None,
                Some(97.5),
                None,
            ],
        ];
        let view = ScreenView {
            decks: [0, 1].map(|i| DeckView {
                id: [DeckId::A, DeckId::B][i],
                focused: i == 0,
                title: Some(titles[i].into()),
                bpm: Some(128.0),
                key: Some(["8A", "9A"][i].into()),
                position_secs: positions[i],
                duration_secs: decks[i].secs,
                loading: false,
                playing: true,
                hot_cues: hot_cue_secs[i].map(|c| c.is_some()),
                waveform: decks[i].ranges.clone(),
                bands: decks[i].bands.clone(),
                waveform_mode: mode,
                beat: Some([(22, 3), (54, 1)][i]),
                cue_secs: Some([15.0, 7.5][i]),
                hot_cue_secs: hot_cue_secs[i],
                // Deck A shows a four bar loop around its playhead.
                loop_secs: (i == 0).then_some((28.0, 43.0)),
                loop_in_secs: None,
                quantize: i == 0,
                key_lock: i == 1,
                end_warning: i == 1,
            }),
            mixer: MixerView {
                crossfader: -0.3,
                faders: [1.0, 0.7],
                headphone_cue: [false, true],
                cue_mix: 0.4,
                strips: [
                    StripView { trim_db: 0.0, eq_db: [0.0, 0.0, 2.0], kills: [false; 3], filter: 0.0, meter: 0.8,
                        fx_name: FxKind::Echo.name(), fx_on: true, fx_wet: 0.6, fade_target: None },
                    StripView { trim_db: -2.0, eq_db: [0.0, -6.0, 0.0], kills: [true, false, false], filter: 0.3, meter: 0.35,
                        fx_name: FxKind::Reverb.name(), fx_on: false, fx_wet: 0.0, fade_target: None },
                ],
                master_meter: 0.85,
                fade_beats: 8.0,
                crossfader_fade_target: None,
                focused: DeckId::A,
            },
            status: "JACK dj-tui @ 48000 Hz / 256 frames, xruns 0  |  hold-cue on  |  pixel waveforms  |  ? help".into(),
            message: format!("Waveform: {}", match mode { WaveformMode::ThreeBand => "3-Band", WaveformMode::Rgb => "RGB", WaveformMode::Blue => "Blue" }),
            phase: Some(0.12),
            help: false,
            browser: Default::default(),
            devices: None,
        };
        let mut term = Terminal::new(TestBackend::new(COLS, ROWS)).unwrap();
        term.draw(|f| render_screen(f, &view)).unwrap();
        let buf = term.backend().buffer();
        let mut cells = std::fs::File::create(format!("{out}/{name}.cells")).unwrap();
        for y in 0..ROWS {
            for x in 0..COLS {
                let c = &buf[(x, y)];
                writeln!(
                    cells,
                    "{x}\t{y}\t{}\t{:?}\t{:?}\t{:?}",
                    c.symbol().replace('\t', " "),
                    c.fg,
                    c.bg,
                    c.modifier
                )
                .unwrap();
            }
        }
        let layout = screen_layout(Rect::new(0, 0, COLS, ROWS));
        for (i, panel) in [layout.deck_a, layout.deck_b].into_iter().enumerate() {
            let area = waveform_area(panel);
            let (w, h) = (area.width as u32 * CELL.0, area.height as u32 * CELL.1);
            let wave = Wave {
                ranges: &decks[i].ranges,
                bands: &decks[i].bands,
                mode,
                warning: i == 1,
                // The same loop the deck view shows, so both renderers appear in a shot.
                loop_cols: loop_columns(view.decks[i].loop_secs, decks[i].secs, w),
                focused: view.decks[i].focused,
            };
            let pal = Palette::default();
            let img = WaveformBitmaps::rasterize(&wave, w, h, &pal).compose_with(
                playhead_x(positions[i], decks[i].secs, w),
                wave.warning,
                wave.loop_cols,
                &pal,
            );
            let mut f = std::fs::File::create(format!("{out}/{name}_deck{i}.rgba")).unwrap();
            writeln!(f, "{w} {h} {} {}", area.x, area.y).unwrap();
            f.write_all(img.as_raw()).unwrap();
        }
    }
}

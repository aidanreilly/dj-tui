//! Render a track's overview waveform to PNGs, one per colour mode.
//!
//! A development aid for tuning the band colours by eye without a terminal:
//!
//! ```sh
//! cargo run --example waveform_preview -- some.flac /tmp/out
//! ```
//!
//! Writes `<out>-3band.png`, `<out>-rgb.png` and `<out>-blue.png`.

use tui::pixel::{Palette, WaveformBitmaps};
use wave::WaveformMode;

fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(path), Some(out)) = (args.next(), args.next()) else {
        eprintln!("usage: waveform_preview <audio file> <output prefix>");
        std::process::exit(2);
    };

    let loaded = match loader::load_file(std::path::Path::new(&path), 48_000, None) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("could not load {path}: {e}");
            std::process::exit(1);
        }
    };
    println!("{} — {} points", loaded.title, loaded.waveform.len());

    let summary = |label: &str, f: fn(&wave::WavePoint) -> f32| {
        let mut v: Vec<f32> = loaded.waveform.iter().map(f).collect();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let at = |q: f32| v[((v.len() - 1) as f32 * q) as usize];
        println!(
            "  {label:>4}: median {:.3}  p90 {:.3}  max {:.3}",
            at(0.5),
            at(0.9),
            at(1.0)
        );
    };
    summary("low", |p| p.bands[0]);
    summary("mid", |p| p.bands[1]);
    summary("high", |p| p.bands[2]);

    for (mode, name) in [
        (WaveformMode::ThreeBand, "3band"),
        (WaveformMode::Rgb, "rgb"),
        (WaveformMode::Blue, "blue"),
    ] {
        let palette = Palette::for_mode(mode);
        let bitmaps = WaveformBitmaps::rasterize(&loaded.waveform, 1200, 200, &palette);
        // Flatten onto black so the PNG reads the way a terminal shows it.
        let mut img = image::RgbImage::new(1200, 200);
        for (x, y, p) in bitmaps.normal().enumerate_pixels() {
            let a = p[3] as f32 / 255.0;
            img.put_pixel(
                x,
                y,
                image::Rgb([
                    (p[0] as f32 * a) as u8,
                    (p[1] as f32 * a) as u8,
                    (p[2] as f32 * a) as u8,
                ]),
            );
        }
        let file = format!("{out}-{name}.png");
        img.save(&file).expect("write png");
        println!("wrote {file}");
    }
}

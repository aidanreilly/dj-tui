//! Run the tempo and key detectors over an annotated dataset and report accuracy (spec 6).
//!
//!     cargo run --release --example giantsteps -- /path/to/giantsteps [--min-tempo 90]
//!
//! Every audio file is decoded, analysed and scored against its annotation. Annotations are
//! looked for beside the audio as `<file>.bpm` and `<file>.key`, then in `annotations/tempo`
//! and `annotations/key` next to it, which is how the GiantSteps sets ship. The exit status is
//! non-zero when accuracy falls below the thresholds, so CI can run this unattended.

use analysis::eval::{key_score, parse_key_annotation, tempo_verdict, Accuracy, TempoVerdict};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Spec 3.5: 90 % of tempos within ±2 %, 65 % of keys exactly right.
const TEMPO_TARGET: f64 = 0.90;
const KEY_TARGET: f64 = 0.65;

struct Args {
    root: PathBuf,
    tempo_target: f64,
    key_target: f64,
    verbose: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut root = None;
    let mut tempo_target = TEMPO_TARGET;
    let mut key_target = KEY_TARGET;
    let mut verbose = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--tempo-target" | "--key-target" => {
                let value: f64 = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or_else(|| format!("{arg} needs a number from 0 to 1"))?;
                if arg == "--tempo-target" {
                    tempo_target = value;
                } else {
                    key_target = value;
                }
            }
            "--verbose" | "-v" => verbose = true,
            other if other.starts_with('-') => return Err(format!("unknown option {other}")),
            other => root = Some(PathBuf::from(other)),
        }
    }
    Ok(Args {
        root: root.ok_or("pass the dataset directory")?,
        tempo_target,
        key_target,
        verbose,
    })
}

fn audio_files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&next) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Annotations live in their own directories; only audio is analysed.
                if path.file_name().is_some_and(|n| n == "annotations") {
                    continue;
                }
                stack.push(path);
            } else if matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("wav" | "flac" | "mp3" | "m4a" | "ogg")
            ) {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// The annotation for `audio`, beside it or under `annotations/<kind>`.
fn annotation(audio: &Path, kind: &str, extension: &str) -> Option<String> {
    let stem = audio.file_stem()?.to_str()?;
    // GiantSteps names files `<id>.LOFI.wav` against annotations named `<id>.LOFI.<ext>`.
    let mut candidates = vec![audio.with_extension(extension)];
    if let Some(parent) = audio.parent() {
        candidates.push(
            parent
                .join("annotations")
                .join(kind)
                .join(format!("{stem}.{extension}")),
        );
        if let Some(grandparent) = parent.parent() {
            candidates.push(
                grandparent
                    .join("annotations")
                    .join(kind)
                    .join(format!("{stem}.{extension}")),
            );
        }
    }
    candidates
        .iter()
        .find_map(|path| std::fs::read_to_string(path).ok())
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(e) => {
            eprintln!("{e}");
            eprintln!("usage: giantsteps <dataset dir> [--tempo-target 0.9] [--key-target 0.65]");
            return ExitCode::FAILURE;
        }
    };
    let files = audio_files(&args.root);
    if files.is_empty() {
        eprintln!("no audio under {}", args.root.display());
        return ExitCode::FAILURE;
    }
    println!("{} tracks under {}", files.len(), args.root.display());

    let mut acc = Accuracy::default();
    let mut failures = Vec::new();
    for (i, path) in files.iter().enumerate() {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let loaded = match loader::load_file(path, 48_000) {
            Ok(loaded) => loaded,
            Err(e) => {
                eprintln!("  {name}: {e}");
                continue;
            }
        };
        if let Some(truth) = annotation(path, "tempo", "bpm").and_then(|t| t.trim().parse().ok()) {
            let estimate = loaded.grid.map_or(0.0, |g| g.bpm);
            let verdict = tempo_verdict(estimate, truth);
            acc.add_tempo(verdict);
            if verdict != TempoVerdict::Exact {
                failures.push(format!(
                    "{name}: {estimate:.2} BPM against {truth:.2} ({verdict:?})"
                ));
            }
        }
        if let Some(truth) = annotation(path, "key", "key").and_then(|t| parse_key_annotation(&t)) {
            let score = loaded.key.map_or(0.0, |k| key_score(k, truth));
            acc.add_key(score);
            if score < 1.0 {
                let estimate = loaded
                    .key
                    .map(|k| k.name())
                    .unwrap_or_else(|| "none".into());
                failures.push(format!(
                    "{name}: {estimate} against {} ({score:.1})",
                    truth.name()
                ));
            }
        }
        if args.verbose || (i + 1) % 50 == 0 {
            println!("  {}/{}", i + 1, files.len());
        }
    }

    println!();
    println!("tempo: {} tracks", acc.tempo_tracks);
    println!("  within 2%      {:.1}%", acc.tempo_exact() * 100.0);
    println!(
        "  allowing octave {:.1}%",
        acc.tempo_within_octave() * 100.0
    );
    println!("key: {} tracks", acc.key_tracks);
    println!("  exact          {:.1}%", acc.key_exact() * 100.0);
    println!("  MIREX weighted {:.1}%", acc.key_weighted() * 100.0);

    if args.verbose {
        println!();
        for line in &failures {
            println!("  {line}");
        }
    }

    let tempo_ok = acc.tempo_tracks == 0 || acc.tempo_exact() >= args.tempo_target;
    let key_ok = acc.key_tracks == 0 || acc.key_exact() >= args.key_target;
    if tempo_ok && key_ok {
        ExitCode::SUCCESS
    } else {
        println!();
        if !tempo_ok {
            println!(
                "tempo accuracy is under the {:.0}% target",
                args.tempo_target * 100.0
            );
        }
        if !key_ok {
            println!(
                "key accuracy is under the {:.0}% target",
                args.key_target * 100.0
            );
        }
        ExitCode::FAILURE
    }
}

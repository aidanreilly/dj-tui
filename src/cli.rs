//! Command line: `dj-tui [--demo] [--no-audio] [FILE_A] [FILE_B]`.

use std::path::PathBuf;

pub const USAGE: &str = "\
usage: dj-tui [OPTIONS] [FILE_A] [FILE_B]

Loads FILE_A on deck A and FILE_B on deck B.

options:
  --demo       use supplied files, or 124/126 BPM click tracks if no files are given
  --no-audio   run without a sound server (silent clock)
  -h, --help   show this help
  --version    show the version";

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Args {
    pub files: Vec<PathBuf>,
    pub demo: bool,
    pub no_audio: bool,
    pub help: bool,
    pub version: bool,
}

pub fn parse_args(args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut out = Args::default();
    let mut flags_done = false;
    for arg in args {
        if !flags_done && arg.starts_with('-') {
            match arg.as_str() {
                "--" => flags_done = true,
                "--demo" => out.demo = true,
                "--no-audio" => out.no_audio = true,
                "-h" | "--help" => out.help = true,
                "--version" => out.version = true,
                other => return Err(format!("unknown option {other}\n\n{USAGE}")),
            }
        } else {
            out.files.push(PathBuf::from(arg));
        }
    }
    if out.files.len() > 2 {
        return Err(format!(
            "at most two files (one per deck), got {}",
            out.files.len()
        ));
    }
    Ok(out)
}

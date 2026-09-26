use dj_tui::cli::{parse_args, Args};
use std::path::PathBuf;

fn parse(v: &[&str]) -> Result<Args, String> {
    parse_args(v.iter().map(|s| s.to_string()))
}

#[test]
fn no_arguments_is_an_empty_session() {
    assert_eq!(parse(&[]).unwrap(), Args::default());
}

#[test]
fn up_to_two_files_go_to_decks_a_and_b() {
    let a = parse(&["one.flac", "two.mp3"]).unwrap();
    assert_eq!(a.files, vec![PathBuf::from("one.flac"), PathBuf::from("two.mp3")]);
    assert!(parse(&["1", "2", "3"]).unwrap_err().contains("two"));
}

#[test]
fn flags_are_recognised_in_any_position() {
    let a = parse(&["x.wav", "--no-audio", "--demo"]).unwrap();
    assert!(a.no_audio && a.demo);
    assert_eq!(a.files.len(), 1);
}

#[test]
fn help_and_version() {
    assert!(parse(&["--help"]).unwrap().help);
    assert!(parse(&["-h"]).unwrap().help);
    assert!(parse(&["--version"]).unwrap().version);
}

#[test]
fn unknown_flags_are_errors() {
    assert!(parse(&["--turbo"]).unwrap_err().contains("--turbo"));
}

#[test]
fn double_dash_ends_flag_parsing() {
    let a = parse(&["--", "--weird-name.wav"]).unwrap();
    assert_eq!(a.files, vec![PathBuf::from("--weird-name.wav")]);
}

use tui::multiplexer_detected;

fn env(vars: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    move |k| {
        vars.iter()
            .find(|(n, _)| *n == k)
            .map(|(_, v)| v.to_string())
    }
}

#[test]
fn plain_ghostty_is_not_a_multiplexer() {
    assert!(!multiplexer_detected(env(&[
        ("TERM", "xterm-ghostty"),
        ("TERM_PROGRAM", "ghostty")
    ])));
}

#[test]
fn tmux_and_screen_are_detected() {
    assert!(multiplexer_detected(env(&[(
        "TMUX",
        "/tmp/tmux-1000/default,1,0"
    )])));
    assert!(multiplexer_detected(env(&[("TERM", "screen-256color")])));
    assert!(multiplexer_detected(env(&[("STY", "1234.pts-0")])));
    assert!(multiplexer_detected(env(&[("ZELLIJ", "0")])));
}

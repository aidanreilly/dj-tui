pub mod app;
pub mod apply;
pub mod automation;
pub mod browser;
pub mod cli;
pub mod clock;
pub mod config;
pub mod controller;
pub mod demo;
pub mod log;
pub mod session;
pub mod view;

/// `$XDG_STATE_HOME/dj-tui/discogs.json`, falling back to `~/.local/state`, beside the log
/// and the session file. The cache is about questions asked rather than about any one track,
/// so it does not belong next to the music and survives a library moving.
pub fn discogs_cache_path(
    state_home: Option<&str>,
    home: Option<&str>,
) -> Option<std::path::PathBuf> {
    let dir = match state_home.filter(|s| !s.is_empty()) {
        Some(state) => std::path::PathBuf::from(state),
        None => std::path::PathBuf::from(home?).join(".local/state"),
    };
    Some(dir.join("dj-tui").join("discogs.json"))
}

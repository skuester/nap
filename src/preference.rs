//! The chosen visualizer, remembered between launches as one word in `$XDG_STATE_HOME/nap/visualizer`.

use std::path::{Path, PathBuf};

/// In the order a click cycles through them; the first is the default.
pub const VISUALIZERS: [&str; 5] = ["bars", "vu", "peak", "spectrogram", "scope"];

pub fn state_file(state_dir: Option<&Path>) -> Option<PathBuf> {
    state_dir.map(|dir| dir.join("nap/visualizer"))
}

fn user_state_file() -> Option<PathBuf> {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")));
    state_file(state.as_deref())
}

/// The saved choice; anything missing or unrecognised means the default.
pub fn read(file: Option<&Path>) -> &'static str {
    let saved = file.and_then(|f| std::fs::read_to_string(f).ok()).unwrap_or_default();
    VISUALIZERS.into_iter().find(|name| *name == saved.trim()).unwrap_or(VISUALIZERS[0])
}

pub fn after(current: &str) -> &'static str {
    let index = VISUALIZERS.iter().position(|name| *name == current).unwrap_or(0);
    VISUALIZERS[(index + 1) % VISUALIZERS.len()]
}

pub fn write(file: &Path, name: &str) -> Result<(), String> {
    let problem = |e: std::io::Error| format!("cannot remember the visualizer in {}: {e}", file.display());
    std::fs::create_dir_all(file.parent().unwrap_or(Path::new("."))).map_err(problem)?;
    std::fs::write(file, format!("{name}\n")).map_err(problem)
}

pub fn load() -> &'static str {
    read(user_state_file().as_deref())
}

/// Advance to the next visualizer and remember it. The switch happens even if saving fails.
pub fn advance(current: &str) -> (&'static str, Result<(), String>) {
    let next = after(current);
    let saved = user_state_file().ok_or("no state directory".to_string()).and_then(|file| write(&file, next));
    (next, saved)
}

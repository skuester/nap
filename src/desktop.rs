//! Symlink-based user installation and reversible MIME defaults.
use crate::install;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

pub const MIME_TYPES: &[&str] = &[
    "audio/mpeg",
    "audio/flac",
    "audio/x-wav",
    "audio/ogg",
    "audio/opus",
    "audio/mp4",
    "audio/aac",
    "audio/x-aiff",
    "audio/x-ms-wma",
];
pub const DESKTOP: &str = "nap.desktop";

pub fn config_home() -> PathBuf {
    home_dir("XDG_CONFIG_HOME", ".config")
}
pub fn state_home() -> PathBuf {
    home_dir("XDG_STATE_HOME", ".local/state")
}
fn home_dir(key: &str, suffix: &str) -> PathBuf {
    std::env::var_os(key)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(suffix))
}
pub fn prefix() -> PathBuf {
    home_dir("NAP_PREFIX", ".local")
}

pub trait MimeBackend {
    fn current(&self, mime: &str) -> Result<String, String>;
    fn set(&self, mime: &str, handler: &str) -> Result<(), String>;
}

pub struct XdgMime {
    pub config: PathBuf,
}
impl MimeBackend for XdgMime {
    fn current(&self, mime: &str) -> Result<String, String> {
        let out = Command::new("xdg-mime").args(["query", "default", mime]).output().map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).into());
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().into())
    }
    fn set(&self, mime: &str, handler: &str) -> Result<(), String> {
        if handler.is_empty() {
            let file = self.config.join("mimeapps.list");
            if file.exists() {
                let old = fs::read_to_string(&file).map_err(|e| e.to_string())?;
                let new = remove_default(&old, mime);
                fs::write(file, new).map_err(|e| e.to_string())?;
            }
            return Ok(());
        }
        let status = Command::new("xdg-mime").args(["default", handler, mime]).status().map_err(|e| e.to_string())?;
        if status.success() { Ok(()) } else { Err(format!("xdg-mime failed for {mime}")) }
    }
}

pub fn remove_default(text: &str, mime: &str) -> String {
    let mut defaults = false;
    text.lines()
        .filter(|line| {
            if line.starts_with('[') {
                defaults = line.trim() == "[Default Applications]";
            }
            !(defaults
                && line
                    .split_once('=')
                    .is_some_and(|(key, value)| key == mime && value.trim_end_matches(';') == DESKTOP))
        })
        .map(|line| format!("{line}\n"))
        .collect()
}

fn read_previous(file: &Path) -> Result<BTreeMap<String, String>, String> {
    match fs::read(file) {
        Ok(data) => serde_json::from_slice(&data).map_err(|e| format!("{}: {e}", file.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(e) => Err(e.to_string()),
    }
}
fn save_previous(file: &Path, previous: &BTreeMap<String, String>) -> Result<(), String> {
    fs::create_dir_all(file.parent().ok_or("missing state directory")?).map_err(|e| e.to_string())?;
    let temp = file.with_extension("json.tmp");
    fs::write(&temp, serde_json::to_vec(previous).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    fs::rename(temp, file).map_err(|e| e.to_string())
}
pub fn install_mimes(backend: &impl MimeBackend, file: &Path) -> Result<(), String> {
    let mut previous = read_previous(file)?;
    for &mime in MIME_TYPES {
        let current = backend.current(mime)?;
        if current == DESKTOP {
            continue;
        }
        previous.insert(mime.into(), current);
        // Persist each displaced default before mutating it, including partial failures.
        save_previous(file, &previous)?;
        backend.set(mime, DESKTOP)?;
    }
    Ok(())
}
pub fn uninstall_mimes(backend: &impl MimeBackend, file: &Path) -> Result<(), String> {
    let previous = read_previous(file)?;
    for (mime, handler) in previous {
        if backend.current(&mime)? == DESKTOP {
            backend.set(&mime, &handler)?;
        }
    }
    if file.exists() {
        fs::remove_file(file).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn symlink(source: &Path, destination: &Path) -> Result<(), String> {
    let source = source.canonicalize().map_err(|e| e.to_string())?;
    if fs::read_link(destination).ok().as_ref() == Some(&source) {
        return Ok(());
    }
    if fs::symlink_metadata(destination).is_ok() {
        if destination.is_dir() {
            return Err(format!("{} is a directory", destination.display()));
        }
        fs::remove_file(destination).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(destination.parent().ok_or("missing install directory")?).map_err(|e| e.to_string())?;
    std::os::unix::fs::symlink(source, destination).map_err(|e| e.to_string())
}
pub fn install_desktop(
    checkout: &Path,
    prefix: &Path,
    config: &Path,
    state: &Path,
    backend: &impl MimeBackend,
) -> Result<(), String> {
    // Check all required sources before changing the installation.
    for file in ["target/release/nap", "nap.desktop", "hypr/nap.lua"] {
        if !checkout.join(file).is_file() {
            return Err(format!("missing {file}; build the release binary first"));
        }
    }
    if !config.join("hypr/hyprland.lua").is_file() {
        return Err("missing Hyprland Lua configuration".into());
    }
    install::install(&config.join("hypr"), install::Source::Link(checkout.join("hypr/nap.lua")))?;
    symlink(&checkout.join("target/release/nap"), &prefix.join("bin/nap"))?;
    symlink(&checkout.join("nap.desktop"), &prefix.join("share/applications/nap.desktop"))?;
    install_mimes(backend, &state.join("nap/previous-audio-handlers.json"))
}
pub fn uninstall_desktop(prefix: &Path, config: &Path, state: &Path, backend: &impl MimeBackend) -> Result<(), String> {
    uninstall_mimes(backend, &state.join("nap/previous-audio-handlers.json"))?;
    install::uninstall(&config.join("hypr"))?;
    for file in [prefix.join("bin/nap"), prefix.join("share/applications/nap.desktop")] {
        if fs::symlink_metadata(&file).is_ok() {
            fs::remove_file(file).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub fn reload_hyprland() -> Result<(), String> {
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
        return Ok(());
    }
    let reload = Command::new("hyprctl").arg("reload").output().map_err(|e| e.to_string())?;
    if !reload.status.success() {
        return Err("hyprctl reload failed".into());
    }
    let errors = Command::new("hyprctl").arg("configerrors").output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&errors.stdout);
    if !errors.status.success() || !text.trim().is_empty() {
        return Err(format!("Hyprland configuration errors: {text}"));
    }
    Ok(())
}

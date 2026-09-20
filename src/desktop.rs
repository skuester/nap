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
    // nap's own: a mixtape archive and its standalone track listing, defined in nap-mime.xml.
    "application/x-nap-tape",
    "application/x-nap-jcard",
];
pub const DESKTOP: &str = "nap.desktop";
/// Where the definitions of nap's own types are installed, under the prefix.
pub const MIME_PACKAGE: &str = "share/mime/packages/nap.xml";

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
    /// Rebuild the desktop's caches under `share` (a prefix's `share` directory) after nap's type
    /// definitions or desktop entry change: what a `.tape` is, and that nap opens one.
    fn refresh(&self, share: &Path) -> Result<(), String>;
}

pub struct XdgMime {
    pub config: PathBuf,
    /// The `xdg-mime` to run; tests substitute a stand-in.
    pub program: PathBuf,
    /// Likewise the `update-mime-database`.
    pub database_program: PathBuf,
    /// And the `update-desktop-database`, which file managers rely on to know what nap opens.
    pub desktop_database_program: PathBuf,
}

fn rebuild(program: &Path, directory: &Path) -> Result<(), String> {
    let name = program.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let status = Command::new(program).arg(directory).status().map_err(|e| format!("{name}: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("{name} failed")) }
}
impl XdgMime {
    pub fn new(config: PathBuf) -> Self {
        XdgMime {
            config,
            program: "xdg-mime".into(),
            database_program: "update-mime-database".into(),
            desktop_database_program: "update-desktop-database".into(),
        }
    }

    /// Forget the default for `mime`: xdg-mime has no command for it, so edit `mimeapps.list`.
    fn forget(&self, mime: &str) -> Result<(), String> {
        let file = self.config.join("mimeapps.list");
        if !file.exists() {
            return Ok(());
        }
        let old = fs::read_to_string(&file).map_err(|e| e.to_string())?;
        fs::write(file, remove_default(&old, mime)).map_err(|e| e.to_string())
    }
}
impl MimeBackend for XdgMime {
    fn current(&self, mime: &str) -> Result<String, String> {
        let out = Command::new(&self.program).args(["query", "default", mime]).output().map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).into());
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().into())
    }
    fn set(&self, mime: &str, handler: &str) -> Result<(), String> {
        if handler.is_empty() {
            return self.forget(mime);
        }
        let status =
            Command::new(&self.program).args(["default", handler, mime]).status().map_err(|e| e.to_string())?;
        if status.success() { Ok(()) } else { Err(format!("xdg-mime failed for {mime}")) }
    }
    fn refresh(&self, share: &Path) -> Result<(), String> {
        rebuild(&self.database_program, &share.join("mime"))?;
        rebuild(&self.desktop_database_program, &share.join("applications"))
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
        if current != DESKTOP {
            previous.insert(mime.into(), current);
            // Persist each displaced default before mutating it, including partial failures.
            save_previous(file, &previous)?;
        }
        // Always write the choice down. A query can answer "nap" from the desktop entry alone, with
        // nothing recorded, and file managers that read the record would then pick something else.
        backend.set(mime, DESKTOP)?;
    }
    Ok(())
}
/// Hand `mime` to `handler` (or to nobody, when that is empty) if nap is still what opens it.
fn release(backend: &impl MimeBackend, mime: &str, handler: &str) -> Result<(), String> {
    if backend.current(mime)? == DESKTOP { backend.set(mime, handler) } else { Ok(()) }
}

pub fn uninstall_mimes(backend: &impl MimeBackend, file: &Path) -> Result<(), String> {
    for (mime, handler) in read_previous(file)? {
        release(backend, &mime, &handler)?;
    }
    // nap's own types had no handler before nap, so there is nothing to restore: just forget them.
    for mime in MIME_TYPES.iter().filter(|mime| !mime.starts_with("audio/")) {
        release(backend, mime, "")?;
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
    for file in ["target/release/nap", "nap.desktop", "nap-mime.xml", "hypr/nap.lua"] {
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
    // The desktop must know what a .tape is before nap can become its default.
    symlink(&checkout.join("nap-mime.xml"), &prefix.join(MIME_PACKAGE))?;
    backend.refresh(&prefix.join("share"))?;
    install_mimes(backend, &state.join("nap/previous-audio-handlers.json"))
}
pub fn uninstall_desktop(prefix: &Path, config: &Path, state: &Path, backend: &impl MimeBackend) -> Result<(), String> {
    uninstall_mimes(backend, &state.join("nap/previous-audio-handlers.json"))?;
    install::uninstall(&config.join("hypr"))?;
    let package = prefix.join(MIME_PACKAGE);
    let defined = fs::symlink_metadata(&package).is_ok();
    for file in [prefix.join("bin/nap"), prefix.join("share/applications/nap.desktop"), package] {
        if fs::symlink_metadata(&file).is_ok() {
            fs::remove_file(file).map_err(|e| e.to_string())?;
        }
    }
    // Forget the types too, but only if this prefix ever defined them.
    if defined { backend.refresh(&prefix.join("share")) } else { Ok(()) }
}

pub fn reload_hyprland() -> Result<(), String> {
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
        return Ok(());
    }
    reload_hyprland_with(Path::new("hyprctl"))
}

/// Reload a running Hyprland through `hyprctl` and report any configuration errors it then has.
pub fn reload_hyprland_with(hyprctl: &Path) -> Result<(), String> {
    let reload = Command::new(hyprctl).arg("reload").output().map_err(|e| e.to_string())?;
    if !reload.status.success() {
        return Err("hyprctl reload failed".into());
    }
    let errors = Command::new(hyprctl).arg("configerrors").output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&errors.stdout);
    if !errors.status.success() || !text.trim().is_empty() {
        return Err(format!("Hyprland configuration errors: {text}"));
    }
    Ok(())
}

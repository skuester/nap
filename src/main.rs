use nap::{
    cli::{self, Action, Options},
    desktop, install,
};
use std::{ffi::CString, path::PathBuf, process::ExitCode};

unsafe extern "C" {
    fn nap_run(options: *const std::ffi::c_char) -> std::ffi::c_int;
}

fn play(options: &Options) -> Result<i32, String> {
    for path in &options.paths {
        if !path.is_file() {
            return Err(format!("not a readable file: {}", path.display()));
        }
        std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    let json = CString::new(serde_json::to_string(options).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    // SAFETY: JSON remains alive until the synchronous Qt event loop returns.
    Ok(unsafe { nap_run(json.as_ptr()) })
}

/// Install (`Some`, optionally linking a rules file) or remove (`None`) the Hyprland rules.
fn hyprland(install: Option<Option<PathBuf>>) -> Result<&'static str, String> {
    let hypr = desktop::config_home().join("hypr");
    let done = match install {
        Some(link) => {
            install::install(&hypr, link.map(install::Source::Link).unwrap_or(install::Source::Embedded))?;
            "nap Hyprland rules installed"
        }
        None => {
            install::uninstall(&hypr)?;
            "nap Hyprland rules removed"
        }
    };
    desktop::reload_hyprland().map(|()| done)
}

/// Install from a checkout (`Some`) or remove (`None`) the desktop integration.
fn desktop_integration(checkout: Option<PathBuf>) -> Result<&'static str, String> {
    let (config, prefix, state) = (desktop::config_home(), desktop::prefix(), desktop::state_home());
    let mime = desktop::XdgMime::new(config.clone());
    let done = match checkout {
        Some(checkout) => {
            desktop::install_desktop(&checkout, &prefix, &config, &state, &mime)?;
            "nap installed: checkout symlinks, Hyprland rules, and audio associations"
        }
        None => {
            desktop::uninstall_desktop(&prefix, &config, &state, &mime)?;
            "nap removed; previous audio associations restored"
        }
    };
    desktop::reload_hyprland().map(|()| done)
}

fn run() -> Result<i32, String> {
    let done = match cli::parse(std::env::args_os().skip(1))? {
        Action::Help => cli::HELP.trim_end().to_owned(),
        Action::Version => format!("nap {}", env!("CARGO_PKG_VERSION")),
        Action::MimeTypes => desktop::MIME_TYPES.join("\n"),
        Action::Run(options) => return play(&options),
        Action::Install(link) => hyprland(Some(link))?.to_owned(),
        Action::Uninstall => hyprland(None)?.to_owned(),
        Action::DesktopInstall(checkout) => desktop_integration(Some(checkout))?.to_owned(),
        Action::DesktopUninstall => desktop_integration(None)?.to_owned(),
    };
    println!("{done}");
    Ok(0)
}
fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code as u8),
        Err(error) => {
            eprintln!("nap: {error}");
            ExitCode::from(2)
        }
    }
}

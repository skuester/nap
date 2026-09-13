use nap::{
    cli::{self, Action},
    desktop, install,
};
use std::{ffi::CString, process::ExitCode};

unsafe extern "C" {
    fn nap_run(options: *const std::ffi::c_char) -> std::ffi::c_int;
}

fn run() -> Result<i32, String> {
    match cli::parse(std::env::args_os().skip(1))? {
        Action::Help => print!("{}", cli::HELP),
        Action::Version => println!("nap {}", env!("CARGO_PKG_VERSION")),
        Action::MimeTypes => {
            for mime in desktop::MIME_TYPES {
                println!("{mime}");
            }
        }
        Action::Run(options) => {
            if let Some(path) = &options.path {
                if !path.is_file() {
                    return Err(format!("not a readable file: {}", path.display()));
                }
                std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
            }
            let json =
                CString::new(serde_json::to_string(&options).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            // SAFETY: JSON remains alive until the synchronous Qt event loop returns.
            return Ok(unsafe { nap_run(json.as_ptr()) });
        }
        Action::Install(link) => {
            install::install(
                &desktop::config_home().join("hypr"),
                link.map(install::Source::Link).unwrap_or(install::Source::Embedded),
            )?;
            desktop::reload_hyprland()?;
            println!("nap Hyprland rules installed");
        }
        Action::Uninstall => {
            install::uninstall(&desktop::config_home().join("hypr"))?;
            desktop::reload_hyprland()?;
            println!("nap Hyprland rules removed");
        }
        Action::DesktopInstall(checkout) => {
            let config = desktop::config_home();
            desktop::install_desktop(
                &checkout,
                &desktop::prefix(),
                &config,
                &desktop::state_home(),
                &desktop::XdgMime { config: config.clone() },
            )?;
            desktop::reload_hyprland()?;
            println!("nap installed: checkout symlinks, Hyprland rules, and audio associations");
        }
        Action::DesktopUninstall => {
            let config = desktop::config_home();
            desktop::uninstall_desktop(
                &desktop::prefix(),
                &config,
                &desktop::state_home(),
                &desktop::XdgMime { config: config.clone() },
            )?;
            desktop::reload_hyprland()?;
            println!("nap removed; previous audio associations restored");
        }
    }
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

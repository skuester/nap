use serde::Serialize;
use std::{ffi::OsString, path::PathBuf};

pub const HELP: &str = "nap — Nice Audio Player

usage: nap [options] [audio]
       nap --install-hyprland [--link path/to/hypr/nap.lua]
       nap --uninstall-hyprland
       nap --mime-types

  --paused                  open paused
  --time, --start, --timestamp T  seconds, m:ss, h:mm:ss; overrides bookmark
  --ignore-bookmark         start at the beginning
  --volume N                initial volume, 0–100 (default 75)
  --loop                    repeat playback
  --install-hyprland        install floating, centered, aspect-preserving rules
  --link PATH               symlink rules from checkout
  --uninstall-hyprland      remove rules and require line
  --mime-types              list supported audio MIME types
  --screenshot PATH         save an offscreen preview and exit
  -h, --help                show help
  -V, --version             show version

keys: Space play/pause; S stop; arrows seek/volume; B bookmark;
Shift+B remove bookmark; Enter return to bookmark; L loop; M mute;
O open; ? help; Q quit.
";

#[derive(Debug, Serialize)]
pub struct Options {
    pub path: Option<PathBuf>,
    pub paused: bool,
    pub start: i64,
    pub ignore: bool,
    pub volume: f64,
    pub looping: bool,
    pub screenshot: Option<PathBuf>,
}
impl Default for Options {
    fn default() -> Self {
        Self { path: None, paused: false, start: -1, ignore: false, volume: 0.75, looping: false, screenshot: None }
    }
}
#[derive(Debug)]
pub enum Action {
    Run(Options),
    Help,
    Version,
    MimeTypes,
    Install(Option<PathBuf>),
    Uninstall,
    DesktopInstall(PathBuf),
    DesktopUninstall,
}

pub fn timestamp(text: &str) -> Option<i64> {
    let parts: Vec<_> = text.split(':').collect();
    if parts.is_empty() || parts.len() > 3 {
        return None;
    }
    let mut seconds = 0.0;
    for (i, part) in parts.iter().enumerate() {
        let n: f64 = part.parse().ok()?;
        if !n.is_finite() || n < 0.0 || (i > 0 && n >= 60.0) || (i + 1 < parts.len() && n.fract() != 0.0) {
            return None;
        }
        seconds = seconds * 60.0 + n;
    }
    if seconds > 1e12 { None } else { Some((seconds * 1000.0).round() as i64) }
}

pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Action, String> {
    let mut args = args.into_iter();
    let mut options = Options::default();
    let mut install = false;
    let mut uninstall = false;
    let mut link = None;
    let mut literal = false;
    while let Some(arg) = args.next() {
        if literal {
            if options.path.replace(arg.into()).is_some() {
                return Err("open one audio file at a time".into());
            }
            continue;
        }
        let text = arg.to_str().unwrap_or("");
        let (key, inline) = text.split_once('=').map_or((text, None), |(k, v)| (k, Some(v)));
        let mut value =
            || inline.map(OsString::from).or_else(|| args.next()).ok_or_else(|| format!("{key} needs a value"));
        match key {
            "-h" | "--help" => return Ok(Action::Help),
            "-V" | "--version" => return Ok(Action::Version),
            "--mime-types" => return Ok(Action::MimeTypes),
            "--install-desktop" => return Ok(Action::DesktopInstall(value()?.into())),
            "--uninstall-desktop" => return Ok(Action::DesktopUninstall),
            "--" => literal = true,
            "--paused" => options.paused = true,
            "--ignore-bookmark" => options.ignore = true,
            "--loop" => options.looping = true,
            "--time" | "--start" | "--timestamp" => {
                options.start = timestamp(value()?.to_str().unwrap_or("")).ok_or("invalid timestamp")?
            }
            "--volume" => {
                let v: f64 = value()?.to_str().unwrap_or("").parse().map_err(|_| "invalid volume")?;
                if !v.is_finite() || !(0.0..=100.0).contains(&v) {
                    return Err("volume must be between 0 and 100".into());
                }
                options.volume = v / 100.0;
            }
            "--screenshot" => options.screenshot = Some(value()?.into()),
            "--install-hyprland" => install = true,
            "--uninstall-hyprland" => uninstall = true,
            "--link" => link = Some(value()?.into()),
            _ if text.starts_with('-') => return Err(format!("unknown option {text}")),
            _ => {
                if options.path.replace(arg.into()).is_some() {
                    return Err("open one audio file at a time".into());
                }
            }
        }
    }
    if install && uninstall {
        return Err("choose install or uninstall".into());
    }
    if link.is_some() && !install {
        return Err("--link requires --install-hyprland".into());
    }
    if (install || uninstall) && options.path.is_some() {
        return Err("install commands do not accept audio files".into());
    }
    if install {
        Ok(Action::Install(link))
    } else if uninstall {
        Ok(Action::Uninstall)
    } else {
        Ok(Action::Run(options))
    }
}

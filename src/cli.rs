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

fn volume(text: &str) -> Result<f64, String> {
    let v: f64 = text.parse().map_err(|_| "invalid volume")?;
    if !v.is_finite() || !(0.0..=100.0).contains(&v) {
        return Err("volume must be between 0 and 100".into());
    }
    Ok(v / 100.0)
}

/// Options that end parsing on the spot.
fn immediate(key: &str) -> Option<Action> {
    match key {
        "-h" | "--help" => Some(Action::Help),
        "-V" | "--version" => Some(Action::Version),
        "--mime-types" => Some(Action::MimeTypes),
        "--uninstall-desktop" => Some(Action::DesktopUninstall),
        _ => None,
    }
}

const VALUED: [&str; 7] =
    ["--time", "--start", "--timestamp", "--volume", "--screenshot", "--link", "--install-desktop"];

#[derive(Default)]
struct Parser {
    options: Options,
    install: bool,
    uninstall: bool,
    link: Option<PathBuf>,
    literal: bool,
}

impl Parser {
    fn file(&mut self, arg: OsString) -> Result<(), String> {
        match self.options.path.replace(arg.into()) {
            Some(_) => Err("open one audio file at a time".into()),
            None => Ok(()),
        }
    }

    /// Switches that take no value; false when `key` is not one.
    fn switch(&mut self, key: &str) -> bool {
        match key {
            "--" => self.literal = true,
            "--paused" => self.options.paused = true,
            "--ignore-bookmark" => self.options.ignore = true,
            "--loop" => self.options.looping = true,
            "--install-hyprland" => self.install = true,
            "--uninstall-hyprland" => self.uninstall = true,
            _ => return false,
        }
        true
    }

    fn valued(&mut self, key: &str, value: OsString) -> Result<Option<Action>, String> {
        let text = value.to_str().unwrap_or("");
        match key {
            "--volume" => self.options.volume = volume(text)?,
            "--screenshot" => self.options.screenshot = Some(value.into()),
            "--link" => self.link = Some(value.into()),
            "--install-desktop" => return Ok(Some(Action::DesktopInstall(value.into()))),
            _ => self.options.start = timestamp(text).ok_or("invalid timestamp")?,
        }
        Ok(None)
    }

    fn finish(self) -> Result<Action, String> {
        if self.install && self.uninstall {
            return Err("choose install or uninstall".into());
        }
        if self.link.is_some() && !self.install {
            return Err("--link requires --install-hyprland".into());
        }
        if (self.install || self.uninstall) && self.options.path.is_some() {
            return Err("install commands do not accept audio files".into());
        }
        Ok(match (self.install, self.uninstall) {
            (true, _) => Action::Install(self.link),
            (_, true) => Action::Uninstall,
            _ => Action::Run(self.options),
        })
    }
}

impl Parser {
    /// One `-` argument, which may draw its value from the arguments after it.
    fn option(&mut self, text: &str, rest: &mut impl Iterator<Item = OsString>) -> Result<Option<Action>, String> {
        let (key, inline) = text.split_once('=').map_or((text, None), |(k, v)| (k, Some(v)));
        if VALUED.contains(&key) {
            let value =
                inline.map(OsString::from).or_else(|| rest.next()).ok_or_else(|| format!("{key} needs a value"))?;
            return self.valued(key, value);
        }
        match immediate(key) {
            None if !self.switch(key) => Err(format!("unknown option {text}")),
            action => Ok(action),
        }
    }
}

pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Action, String> {
    let mut args = args.into_iter();
    let mut parser = Parser::default();
    while let Some(arg) = args.next() {
        let text = arg.to_str().unwrap_or("");
        if parser.literal || !text.starts_with('-') {
            parser.file(arg)?;
        } else if let Some(action) = parser.option(text, &mut args)? {
            return Ok(action);
        }
    }
    parser.finish()
}

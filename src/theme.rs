//! The Omarchy palette. Source of truth is the active theme's `colors.toml`
//! (`~/.local/state/omarchy/current/theme/colors.toml`; older installs kept it
//! under `~/.config`). The file is flat `key = "#rrggbb"` lines, so it is read
//! without a TOML dependency.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Rgb { r, g, b }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let hex = s.trim().strip_prefix('#')?;
        if hex.len() != 6 || !hex.is_ascii() {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        Some(Rgb::new(byte(0)?, byte(2)?, byte(4)?))
    }

    pub fn to_css(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    pub background: Rgb,
    pub foreground: Rgb,
    pub light_foreground: Rgb,
    pub accent: Rgb,
    pub muted: Rgb,
    /// The theme's red (`color1`), for the one thing on a deck that is always red: REC.
    pub red: Rgb,
}

impl Default for Theme {
    /// Catppuccin Mocha, Omarchy's stock theme.
    fn default() -> Self {
        Theme {
            background: Rgb::new(0x1e, 0x1e, 0x2e),
            foreground: Rgb::new(0xcd, 0xd6, 0xf4),
            light_foreground: Rgb::new(0xba, 0xc2, 0xde),
            accent: Rgb::new(0x89, 0xb4, 0xfa),
            muted: Rgb::new(0x58, 0x5b, 0x70),
            red: Rgb::new(0xf3, 0x8b, 0xa8),
        }
    }
}

impl Theme {
    /// One `key = "#rrggbb"` line; comments, other shapes, and bad colours are `None`.
    fn entry(line: &str) -> Option<(&str, Rgb)> {
        let (key, value) = line.trim().split_once('=').filter(|_| !line.trim().starts_with('#'))?;
        Some((key.trim(), Rgb::parse(value.trim().trim_matches('"'))?))
    }

    pub fn parse(text: &str) -> Self {
        let mut t = Theme::default();
        for (key, rgb) in text.lines().filter_map(Theme::entry) {
            match key {
                "background" => t.background = rgb,
                "foreground" => t.foreground = rgb,
                "light_foreground" => t.light_foreground = rgb,
                "accent" => t.accent = rgb,
                "muted" => t.muted = rgb,
                "color1" => t.red = rgb,
                _ => {}
            }
        }
        t
    }

    /// Read a palette file; any problem means the default palette.
    pub fn load(path: Option<&Path>) -> Self {
        path.and_then(|p| std::fs::read_to_string(p).ok()).map(|s| Theme::parse(&s)).unwrap_or_default()
    }

    /// The palette of the active Omarchy theme on this machine.
    pub fn load_omarchy() -> Self {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let state = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|h| h.join(".local/state")));
        let config =
            std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| home.as_ref().map(|h| h.join(".config")));
        Theme::load(colors_toml_path(state.as_deref(), config.as_deref()).as_deref())
    }
}

/// Locate `colors.toml`: the state dir wins over the (older) config dir.
pub fn colors_toml_path(state_dir: Option<&Path>, config_dir: Option<&Path>) -> Option<PathBuf> {
    [state_dir, config_dir]
        .into_iter()
        .flatten()
        .map(|d| d.join("omarchy/current/theme/colors.toml"))
        .find(|p| p.is_file())
}

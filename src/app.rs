//! Playback policy and file state. Qt reports transport state and executes commands.
use crate::{bookmark, insert, theme::Theme};
use serde_json::{Value, json};
use std::path::PathBuf;

pub fn seek(position: i64, duration: i64) -> i64 {
    position.clamp(0, duration.max(0))
}
pub fn skip(position: i64, seconds: i64, duration: i64) -> i64 {
    seek(position.saturating_add(seconds.saturating_mul(1000)), duration)
}
pub fn volume(value: f64) -> f64 {
    if value.is_finite() { value.clamp(0.0, 1.0) } else { 0.0 }
}
/// A label-sized file size in decimal units, like a file manager shows.
pub fn file_size(bytes: u64) -> String {
    match bytes {
        0..1_000 => format!("{bytes} B"),
        1_000..1_000_000 => format!("{} KB", bytes / 1_000),
        1_000_000..1_000_000_000 => format!("{:.1} MB", bytes as f64 / 1e6),
        _ => format!("{:.2} GB", bytes as f64 / 1e9),
    }
}
pub fn start_position(explicit: i64, mark: i64, ignore: bool) -> i64 {
    if explicit >= 0 {
        explicit
    } else if !ignore {
        mark
    } else {
        -1
    }
}

#[derive(Default)]
pub struct App {
    path: Option<PathBuf>,
}
impl App {
    pub fn dispatch(&mut self, request: &Value) -> Result<Value, String> {
        let n = |key: &str| request[key].as_i64().unwrap_or(0);
        let b = |key: &str| request[key].as_bool().unwrap_or(false);
        match request["op"].as_str().unwrap_or("") {
            "open" => {
                let file = PathBuf::from(request["path"].as_str().ok_or("missing path")?);
                let path = file.canonicalize().map_err(|e| format!("Cannot open {}: {e}", file.display()))?;
                if !path.is_file() {
                    return Err("Cannot open: not a regular file".into());
                }
                let size = std::fs::File::open(&path)
                    .and_then(|f| f.metadata())
                    .map_err(|e| format!("Cannot open: {e}"))?
                    .len();
                let mark = bookmark::read(&path).ok().flatten().and_then(|v| i64::try_from(v).ok()).unwrap_or(-1);
                let start = request["start"].as_i64().unwrap_or(-1);
                let pending = start_position(start, mark, b("ignore"));
                self.path = Some(path.clone());
                Ok(json!({"path": path, "mark": mark, "pending": pending, "size": file_size(size),
                    "notice": if start < 0 && !b("ignore") && mark >= 0 { "Opened at your bookmark" } else { "" }}))
            }
            "bookmark" => {
                let path = self.path.as_ref().ok_or("No file loaded")?;
                if b("remove") {
                    bookmark::clear(path)?;
                } else {
                    bookmark::write(path, n("position").max(0) as u64)?;
                }
                Ok(json!({"mark": if b("remove") { -1 } else { n("position").max(0) },
                    "notice": if b("remove") { "Bookmark removed" } else { "Bookmarked" }}))
            }
            "insert" => Ok(insert::read(self.path.as_ref().ok_or("No file loaded")?)),
            "seek" => Ok(json!({"position": seek(n("position"), n("duration"))})),
            "skip" => Ok(json!({"position": skip(n("position"), n("seconds"), n("duration"))})),
            "volume" => Ok(json!({"volume": volume(request["volume"].as_f64().unwrap_or(0.0))})),
            "theme" => {
                let t = Theme::load_omarchy();
                Ok(
                    json!({"background": t.background.to_css(), "foreground": t.foreground.to_css(), "accent": t.accent.to_css()}),
                )
            }
            _ => Err("unknown core operation".into()),
        }
    }
}

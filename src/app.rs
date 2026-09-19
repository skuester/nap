//! Playback policy and file state. Qt reports transport state and executes commands.
use crate::{
    bookmark, insert, preference,
    session::Session,
    theme::Theme,
    transport::{Deck, Event},
};
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
    session: Session,
}
fn number(request: &Value, key: &str) -> i64 {
    request[key].as_i64().unwrap_or(0)
}
fn flag(request: &Value, key: &str) -> bool {
    request[key].as_bool().unwrap_or(false)
}

impl App {
    fn open(&mut self, request: &Value) -> Result<Value, String> {
        let file = PathBuf::from(request["path"].as_str().ok_or("missing path")?);
        let path = file.canonicalize().map_err(|e| format!("Cannot open {}: {e}", file.display()))?;
        if !path.is_file() {
            return Err("Cannot open: not a regular file".into());
        }
        let size =
            std::fs::File::open(&path).and_then(|f| f.metadata()).map_err(|e| format!("Cannot open: {e}"))?.len();
        let mark = bookmark::read(&path).ok().flatten().and_then(|v| i64::try_from(v).ok()).unwrap_or(-1);
        let start = request["start"].as_i64().unwrap_or(-1);
        let ignore = flag(request, "ignore");
        let resumed = start < 0 && !ignore && mark >= 0;
        self.path = Some(path.clone());
        Ok(json!({"path": path, "mark": mark, "pending": start_position(start, mark, ignore), "size": file_size(size),
            "notice": if resumed { "Opened at your bookmark" } else { "" }}))
    }

    fn bookmark(&self, request: &Value) -> Result<Value, String> {
        let path = self.path.as_ref().ok_or("No file loaded")?;
        let position = number(request, "position").max(0);
        if flag(request, "remove") {
            bookmark::clear(path)?;
            return Ok(json!({"mark": -1, "notice": "Bookmark removed"}));
        }
        bookmark::write(path, position as u64)?;
        Ok(json!({"mark": position, "notice": "Bookmarked"}))
    }

    fn visualizer(request: &Value) -> Value {
        if !flag(request, "next") {
            return json!({"name": preference::load()});
        }
        let (name, saved) = preference::advance(request["current"].as_str().unwrap_or(""));
        json!({"name": name, "notice": saved.err().unwrap_or_default()})
    }

    fn transport(request: &Value) -> Result<Value, String> {
        let deck = Deck::parse(request["state"].as_str().unwrap_or(""));
        let event = Event::parse(request["event"].as_str().unwrap_or(""), flag(request, "waiting"));
        Ok(json!({"state": deck.after(event.ok_or("unknown transport event")?).name()}))
    }

    fn insert(&self, request: &Value) -> Result<Value, String> {
        let path = self.session.track_path(request).or(self.path.as_ref()).ok_or("No file loaded")?;
        Ok(insert::read(path))
    }

    /// Requests about the tape in the deck rather than the file under the head.
    fn tape(&mut self, op: &str, request: &Value) -> Option<Result<Value, String>> {
        let session = &mut self.session;
        Some(match op {
            "load" => session.load(request),
            "tape" => Ok(session.describe()),
            "edit" => session.edit(request),
            "export" => session.export(request),
            "job" => Ok(session.poll()),
            "track" => Ok(session.search(flag(request, "forward"), number(request, "position"))),
            "select" => session.select(number(request, "index").max(0) as usize),
            "ended" => Ok(session.ended(flag(request, "looping"))),
            _ => return None,
        })
    }

    fn theme() -> Value {
        let t = Theme::load_omarchy();
        json!({"background": t.background.to_css(), "foreground": t.foreground.to_css(), "accent": t.accent.to_css()})
    }

    pub fn dispatch(&mut self, request: &Value) -> Result<Value, String> {
        let n = |key: &str| number(request, key);
        let op = request["op"].as_str().unwrap_or("");
        if let Some(reply) = self.tape(op, request) {
            return reply;
        }
        match op {
            "open" => self.open(request),
            "bookmark" => self.bookmark(request),
            "insert" => self.insert(request),
            "visualizer" => Ok(Self::visualizer(request)),
            "seek" => Ok(json!({"position": seek(n("position"), n("duration"))})),
            "skip" => Ok(json!({"position": skip(n("position"), n("seconds"), n("duration"))})),
            "volume" => Ok(json!({"volume": volume(request["volume"].as_f64().unwrap_or(0.0))})),
            "theme" => Ok(Self::theme()),
            "transport" => Self::transport(request),
            _ => Err("unknown core operation".into()),
        }
    }
}

//! What is in the deck: the current tape, which of its tracks is up, and any archive being
//! packed in the background. A lone audio file is a tape of one unnamed track.

use crate::insert;
use crate::tape::{self, Brief, Kind, Source, Tape};
use crate::transport;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How long a warning about unsaved work waits for the same thing to be asked again.
pub const SECOND_THOUGHTS: Duration = Duration::from_secs(6);

struct Job {
    label: String,
    total: u64,
    done: Arc<AtomicU64>,
    /// Where the tape was saved, once it has been.
    handle: JoinHandle<Result<PathBuf, String>>,
}

#[derive(Default)]
pub struct Session {
    tape: Tape,
    index: usize,
    dirty: bool,
    job: Option<Job>,
    briefs: HashMap<PathBuf, Brief>,
    /// The cover held inside the tape, as the interface wants it, and which cover that was.
    cover: Option<(PathBuf, String)>,
    /// What was last refused because the tape is unsaved, and when.
    warned: Option<(String, Instant)>,
}

fn paths(request: &Value) -> Vec<PathBuf> {
    request["paths"].as_array().into_iter().flatten().filter_map(Value::as_str).map(PathBuf::from).collect()
}

fn file_name(path: &Path) -> String {
    path.file_name().unwrap_or_default().to_string_lossy().into_owned()
}

impl Session {
    pub fn current(&self) -> Option<&Source> {
        self.tape.tracks.get(self.index)
    }

    /// The track listed at `path`, if the tape has one.
    pub fn listed(&self, path: &Path) -> Option<&Source> {
        self.tape.tracks.iter().find(|track| track.path == path)
    }

    fn track(&self, index: usize) -> Value {
        json!({"index": index, "path": self.tape.tracks.get(index).map(|track| &track.path), "position": 0})
    }

    /// Put a tape in the deck. The reply mentions any tracks it lists that could not be found.
    fn insert_tape(&mut self, (tape, missing): (Tape, Vec<String>)) -> Value {
        (self.tape, self.index, self.dirty) = (tape, 0, false);
        let notice = match missing.len() {
            0 => String::new(),
            1 => format!("Could not find {}", missing[0]),
            n => format!("Could not find {n} of this tape's tracks: {}", missing.join(", ")),
        };
        json!({"notice": notice})
    }

    /// Open `paths` as the new tape, or with `append` add their audio to the current one.
    pub fn load(&mut self, request: &Value) -> Result<Value, String> {
        let paths = paths(request);
        let audio: Vec<Source> =
            paths.iter().filter(|p| matches!(tape::kind(p), Kind::Audio | Kind::Other)).map(Source::from).collect();
        if request["append"].as_bool().unwrap_or(false) {
            self.dirty |= !audio.is_empty();
            self.tape.tracks.extend(audio);
            return Ok(json!({"notice": ""}));
        }
        // A tape is played from where it is, so even an archive is in the deck at once.
        match paths.first().map(|first| (tape::kind(first), first)) {
            Some((Kind::Archive, archive)) => Ok(self.insert_tape(tape::read_archive(archive)?)),
            Some((Kind::Index, index)) => Ok(self.insert_tape(tape::read_index(index)?)),
            _ if audio.is_empty() => Err("Nothing to play there".into()),
            _ => Ok(self.insert_tape((Tape { tracks: audio, ..Tape::default() }, Vec::new()))),
        }
    }

    pub fn export(&mut self, request: &Value) -> Result<Value, String> {
        let dest = PathBuf::from(request["dest"].as_str().ok_or("missing destination")?);
        let dest = if tape::kind(&dest) == Kind::Index { dest } else { dest.with_extension("tape") };
        let (tape, label) = (self.tape.clone(), format!("Saving {}", file_name(&dest)));
        let briefs: Vec<Brief> = tape.tracks.iter().map(|track| self.brief(track)).collect();
        let (done, total) = (Arc::new(AtomicU64::new(0)), tape::export_size(&tape));
        let progress = done.clone();
        let handle = std::thread::spawn(move || tape::export(&tape, &briefs, &dest, &progress).map(|()| dest));
        self.job = Some(Job { label, total, done, handle });
        Ok(json!({"job": true}))
    }

    /// A tape saved over the archive it is played from has moved its tracks about inside it.
    /// Find them again; the file that is playing stays open as it was until the track changes.
    fn relocate(&mut self, dest: &Path) {
        let inside = |source: &Source| source.span.as_ref().is_some_and(|span| span.archive == dest);
        if !self.tape.tracks.iter().chain(self.tape.cover.iter()).any(inside) {
            return;
        }
        match tape::read_archive(dest) {
            Ok((saved, _)) if saved.tracks.len() == self.tape.tracks.len() => {
                (self.tape.tracks, self.tape.cover) = (saved.tracks, saved.cover);
            }
            _ => {}
        }
    }

    fn finish(&mut self, outcome: Result<PathBuf, String>) -> Value {
        match outcome {
            Ok(dest) => {
                self.dirty = false;
                self.relocate(&dest);
                json!({"active": false, "notice": format!("Saved {}", file_name(&dest))})
            }
            Err(error) => json!({"active": false, "notice": error}),
        }
    }

    /// Progress of the background job; the call that finds it finished also applies its result.
    pub fn poll(&mut self) -> Value {
        match self.job.take() {
            None => json!({"active": false, "notice": ""}),
            Some(job) if job.handle.is_finished() => {
                let outcome = job.handle.join().unwrap_or_else(|_| Err("The tape job failed".into()));
                self.finish(outcome)
            }
            Some(job) => {
                let report = json!({"active": true, "label": job.label, "done": job.done.load(Ordering::Relaxed), "total": job.total});
                self.job = Some(job);
                report
            }
        }
    }

    /// What the listing says of a track, read from its tags the first time it is asked for.
    fn brief(&mut self, track: &Source) -> Brief {
        self.briefs.entry(track.path.clone()).or_insert_with(|| insert::brief(track)).clone()
    }

    fn row(&mut self, track: &Source) -> Value {
        let Brief { title, artist, seconds } = self.brief(track);
        json!({"path": track.path, "file": file_name(&track.path), "folder": track.file().parent(),
            "title": title, "artist": artist, "seconds": seconds})
    }

    pub fn describe(&mut self) -> Value {
        let tracks: Vec<Value> = self.tape.tracks.clone().iter().map(|track| self.row(track)).collect();
        let seconds: u64 = tracks.iter().filter_map(|t| t["seconds"].as_u64()).sum();
        let written = [&self.tape.name, &self.tape.from, &self.tape.note].iter().any(|text| !text.is_empty());
        let mixtape = tracks.len() > 1 || written || self.tape.cover.is_some();
        let cover = self.tape.cover.as_ref();
        json!({"name": self.tape.name, "from": self.tape.from, "note": self.tape.note,
            "cover": cover.map(|cover| &cover.path), "coverHeld": cover.is_some_and(|cover| cover.span.is_some()),
            "tracks": tracks, "index": self.index, "seconds": seconds, "dirty": self.dirty, "mixtape": mixtape})
    }

    /// A cover held inside the tape cannot be shown from a path, so it is handed over whole. It
    /// is read once, and asked for only when the cover changes.
    pub fn held_cover(&mut self) -> Value {
        let Some(cover) = self.tape.cover.as_ref().filter(|cover| cover.span.is_some()) else {
            return json!({"url": ""});
        };
        if self.cover.as_ref().is_none_or(|(path, _)| *path != cover.path) {
            self.cover = Some((cover.path.clone(), insert::held_cover(cover)));
        }
        json!({"url": self.cover.as_ref().map(|(_, url)| url)})
    }

    fn remove(&mut self, at: usize) -> Result<bool, String> {
        if at >= self.tape.tracks.len() || self.tape.tracks.len() == 1 {
            return Err("A tape needs at least one track".into());
        }
        self.tape.tracks.remove(at);
        let was_current = at == self.index;
        self.index = (self.index - usize::from(at < self.index)).min(self.tape.tracks.len() - 1);
        Ok(was_current)
    }

    fn reorder(&mut self, from: usize, to: usize) -> Result<bool, String> {
        let last = self.tape.tracks.len().saturating_sub(1);
        if from > last {
            return Err("No such track".into());
        }
        let playing = self.current().cloned();
        let track = self.tape.tracks.remove(from);
        self.tape.tracks.insert(to.min(last), track);
        // Follow the track that is up to wherever it went.
        self.index = self.tape.tracks.iter().position(|t| Some(t) == playing.as_ref()).unwrap_or(self.index);
        Ok(false)
    }

    fn set_cover(&mut self, path: &str) -> Result<bool, String> {
        let cover = PathBuf::from(path);
        if tape::kind(&cover) != Kind::Image || !cover.is_file() {
            return Err("Drop an image to use as the cover".into());
        }
        self.tape.cover = Some(cover.into());
        Ok(false)
    }

    /// Change the tape. The reply says whether the track that is up was taken away.
    pub fn edit(&mut self, request: &Value) -> Result<Value, String> {
        let number = |key: &str| request[key].as_u64().unwrap_or(u64::MAX) as usize;
        let replaced = match request["action"].as_str().unwrap_or("") {
            "remove" => self.remove(number("index"))?,
            "move" => self.reorder(number("from"), number("to"))?,
            "cover" => self.set_cover(request["path"].as_str().unwrap_or(""))?,
            // Words written on the card: its name, who it is from, and the note that goes with it.
            field @ ("name" | "from" | "note") => {
                let text = request["text"].as_str().unwrap_or("").trim().to_owned();
                *match field {
                    "name" => &mut self.tape.name,
                    "from" => &mut self.tape.from,
                    _ => &mut self.tape.note,
                } = text;
                false
            }
            _ => return Err("unknown tape edit".into()),
        };
        self.dirty = true;
        Ok(json!({"replaced": replaced, "path": self.current().map(|track| &track.path)}))
    }

    /// Whether `action` ("open" or "quit") may throw the tape away. Unsaved work is given up only
    /// by asking twice: the first time is refused with a notice, the same request soon after goes
    /// through. No dialog to answer, and nothing lost to one stray key.
    pub fn may_discard(&mut self, action: &str, now: Instant) -> Value {
        let asked_twice = self
            .warned
            .take()
            .is_some_and(|(what, when)| what == action && now.duration_since(when) <= SECOND_THOUGHTS);
        if !self.dirty || asked_twice {
            return json!({"allowed": true, "notice": ""});
        }
        self.warned = Some((action.to_owned(), now));
        let again = if action == "quit" { "Quit" } else { "Open" };
        json!({"allowed": false, "notice": format!("This tape is unsaved. {again} again to discard it, or press REC to save it.")})
    }

    /// PREV or NEXT: move to the track it finds.
    pub fn search(&mut self, forward: bool, position: i64) -> Value {
        let count = self.tape.tracks.len();
        self.index =
            if forward { transport::next(self.index, count) } else { transport::previous(self.index, count, position) };
        self.track(self.index)
    }

    pub fn select(&mut self, index: usize) -> Result<Value, String> {
        if index >= self.tape.tracks.len() {
            return Err("No such track".into());
        }
        self.index = index;
        Ok(self.track(index))
    }

    /// The track that is up ran out: what plays next, if anything.
    pub fn ended(&mut self, looping: bool) -> Value {
        let (index, play) = transport::after_end(self.index, self.tape.tracks.len(), looping);
        self.index = index;
        let mut reply = self.track(index);
        reply["play"] = json!(play);
        reply
    }

    pub fn track_source(&self, request: &Value) -> Option<&Source> {
        request["index"].as_u64().map_or(self.current(), |index| self.tape.tracks.get(index as usize))
    }
}

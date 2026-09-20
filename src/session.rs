//! What is in the deck: the current tape, which of its tracks is up, and any archive being
//! packed in the background. A lone audio file is a tape of one unnamed track.

use crate::bookmark::{self, Mark};
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
/// How long an agreement to quit stands. Quitting is asked more than once on the way out (the
/// key, then the window closing because of it), and the answer must not change in between.
pub const WAY_OUT: Duration = Duration::from_secs(1);

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
    /// The `.tape` or `.jcard` this tape was opened from or last saved to, if it has been either.
    origin: Option<PathBuf>,
    /// The tape's bookmark: a track number and how many milliseconds in, kept on the origin as an
    /// attribute of that file.
    mark: Option<Mark>,
    job: Option<Job>,
    briefs: HashMap<PathBuf, Brief>,
    /// The cover held inside the tape, as the interface wants it, and which cover that was.
    cover: Option<(PathBuf, String)>,
    /// What was last refused because the tape is unsaved, and when.
    warned: Option<(String, Instant)>,
    /// When quitting was last agreed to.
    leaving: Option<Instant>,
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

    /// Put a tape in the deck, with the bookmark its file carries. It comes up where it was left,
    /// unless the request says to `ignore` that, to `start` at some time, or names a `track`
    /// (counted from one); a time alone means the first track. The reply's `cue` is the track to
    /// open and where. It also mentions any listed tracks that could not be found.
    fn insert_tape(&mut self, (tape, missing): (Tape, Vec<String>), origin: Option<&Path>, request: &Value) -> Value {
        (self.tape, self.index, self.dirty, self.origin) = (tape, 0, false, origin.map(Path::to_owned));
        self.mark = origin.and_then(|file| bookmark::read(file).ok().flatten());
        let count = self.tape.tracks.len();
        let asked = request["track"].as_u64().filter(|track| *track > 0).map(|track| track as usize - 1);
        let elsewhere = request["ignore"].as_bool().unwrap_or(false) || request["start"].as_i64().unwrap_or(-1) >= 0;
        let resumed = self.marked().filter(|_| !elsewhere && asked.is_none());
        self.index = resumed.map_or(asked.filter(|track| *track < count).unwrap_or(0), |mark| mark.track);
        let mut notices: Vec<String> = Vec::new();
        // A single file has no tracks to choose between, so there a track number means nothing.
        if let Some(track) = asked.filter(|track| *track >= count && count > 1) {
            notices.push(format!("This tape has only {count} tracks, so track {} is its first instead", track + 1));
        }
        match missing.len() {
            0 => {}
            1 => notices.push(format!("Could not find {}", missing[0])),
            n => notices.push(format!("Could not find {n} of this tape's tracks: {}", missing.join(", "))),
        }
        let position = resumed.map_or(-1, |mark| mark.millisecond as i64);
        json!({"notice": notices.join(". "), "cue": {"path": self.current().map(|track| &track.path),
            "position": position, "resumed": resumed.is_some()}})
    }

    /// Whether the bookmark is the tape's, rather than the one file's that is playing.
    pub fn keeps_mark(&self) -> bool {
        self.origin.is_some() || self.tape.tracks.len() > 1
    }

    /// The bookmark, if it is on a track the tape has. It is a track number and a time and nothing
    /// more, as on the file: a tape is as good as fixed once made, and a number keeps its place
    /// where the same file is listed twice. Move tracks about and it stays with the number.
    fn marked(&self) -> Option<Mark> {
        self.mark.filter(|mark| mark.track < self.tape.tracks.len())
    }

    /// How far into the track that is up the bookmark is, if that is the track it is on.
    pub fn mark_here(&self) -> Option<u64> {
        self.marked().filter(|mark| mark.track == self.index).map(|mark| mark.millisecond)
    }

    /// Put the bookmark, or the lack of one, on `file`.
    fn keep_mark(&self, file: &Path) -> Result<(), String> {
        self.marked().map_or_else(|| bookmark::clear(file), |mark| bookmark::write(file, mark))
    }

    /// Bookmark this `position` in the track that is up, or with none remove the bookmark.
    pub fn bookmark(&mut self, position: Option<u64>) -> Result<Value, String> {
        let origin = self.origin.clone().ok_or("Save the tape first: its bookmark is kept on the tape")?;
        let placed = position.map(|millisecond| Mark { track: self.index, millisecond });
        let before = std::mem::replace(&mut self.mark, placed);
        self.keep_mark(&origin).inspect_err(|_| self.mark = before)?;
        let done = if position.is_some() { "Bookmarked" } else { "Bookmark removed" };
        Ok(json!({"mark": position.map_or(-1, |at| at as i64), "notice": done}))
    }

    /// Go to the bookmark: its track comes up, and the reply says where in it to be.
    pub fn resume(&mut self) -> Value {
        let Some(mark) = self.marked() else { return json!({}) };
        self.index = mark.track;
        json!({"index": mark.track, "path": self.tape.tracks[mark.track].path, "position": mark.millisecond})
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
            Some((Kind::Archive, archive)) => {
                Ok(self.insert_tape(tape::read_archive(archive)?, Some(archive), request))
            }
            Some((Kind::Index, index)) => Ok(self.insert_tape(tape::read_index(index)?, Some(index), request)),
            _ if audio.is_empty() => Err("Nothing to play there".into()),
            _ => Ok(self.insert_tape((Tape { tracks: audio, ..Tape::default() }, Vec::new()), None, request)),
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
                // A saved tape is a new file, so the bookmark is put on it afresh; from now on
                // this is the file the tape is.
                let kept = self.keep_mark(&dest).err().filter(|_| self.mark.is_some());
                self.origin = Some(dest.clone());
                let lost = kept.map(|error| format!(", but not its bookmark: {error}")).unwrap_or_default();
                json!({"active": false, "notice": format!("Saved {}{lost}", file_name(&dest))})
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
            "tracks": tracks, "index": self.index, "seconds": seconds, "dirty": self.dirty, "mixtape": mixtape,
            "markIndex": self.marked().map_or(-1, |mark| mark.track as i64),
            "mark": self.mark_here().map_or(-1, |at| at as i64), "keepsMark": self.keeps_mark(),
            "sideB": self.tape.side_b.map_or(-1, |first| first as i64), "side": self.side()})
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
        self.tape.side_b = transport::side_after_remove(self.tape.side_b, at, self.tape.tracks.len());
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
        self.tape.side_b = transport::side_after_move(self.tape.side_b, from, to.min(last), last + 1);
        // Follow the track that is up to wherever it went.
        self.index = self.tape.tracks.iter().position(|t| Some(t) == playing.as_ref()).unwrap_or(self.index);
        Ok(false)
    }

    /// Start side B at a track, or asked of the track it already starts at, go back to one side.
    fn turn_at(&mut self, at: usize) -> Result<bool, String> {
        let asked = Some(at).filter(|at| self.tape.side_b != Some(*at));
        self.tape.side_b = transport::sided(asked, self.tape.tracks.len());
        if asked.is_some() && self.tape.side_b.is_none() {
            return Err("Side B needs a side A before it".into());
        }
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
            "side" => self.turn_at(number("index"))?,
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
        let leaving = action == "quit" && self.leaving.is_some_and(|agreed| now.duration_since(agreed) <= WAY_OUT);
        if !self.dirty || asked_twice || leaving {
            self.leaving = (action == "quit").then_some(now);
            return json!({"allowed": true, "notice": ""});
        }
        self.warned = Some((action.to_owned(), now));
        let again = if action == "quit" { "Quit" } else { "Open" };
        json!({"allowed": false, "notice": format!("This tape is unsaved. {again} again to discard it, or press REC to save it.")})
    }

    /// Which side is up, on a tape that has two.
    fn side(&self) -> &'static str {
        match self.tape.side_b {
            None => "",
            Some(first) if self.index < first => "A",
            Some(_) => "B",
        }
    }

    /// FLIP: turn the tape over, to the start of its other side.
    pub fn flip(&mut self) -> Result<Value, String> {
        let one_sided = "This tape has one side. Pick out a track in the insert and press F to start side B there";
        self.index = transport::flipped(self.index, self.tape.side_b).ok_or(one_sided)?;
        Ok(self.track(self.index))
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
        let (index, play) = transport::after_end(self.index, self.tape.tracks.len(), looping, self.tape.side_b);
        let turned = !play && index > 0;
        self.index = index;
        let mut reply = self.track(index);
        reply["play"] = json!(play);
        reply["notice"] = json!(if turned { "That was side A. Side B is up; press PLAY" } else { "" });
        reply
    }

    pub fn track_source(&self, request: &Value) -> Option<&Source> {
        request["index"].as_u64().map_or(self.current(), |index| self.tape.tracks.get(index as usize))
    }
}

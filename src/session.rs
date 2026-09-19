//! What is in the deck: the current tape, which of its tracks is up, and any archive being
//! packed or unpacked in the background. A lone audio file is a tape of one unnamed track.

use crate::insert;
use crate::tape::{self, Kind, Tape};
use crate::transport;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::JoinHandle;

enum Outcome {
    Imported(Tape, PathBuf),
    Exported(PathBuf),
}

struct Job {
    label: String,
    total: u64,
    done: Arc<AtomicU64>,
    handle: JoinHandle<Result<Outcome, String>>,
}

#[derive(Default)]
pub struct Session {
    tape: Tape,
    index: usize,
    dirty: bool,
    scratch: Option<PathBuf>,
    job: Option<Job>,
    briefs: HashMap<PathBuf, (String, String, u64)>,
}

fn paths(request: &Value) -> Vec<PathBuf> {
    request["paths"].as_array().into_iter().flatten().filter_map(Value::as_str).map(PathBuf::from).collect()
}

fn file_name(path: &Path) -> String {
    path.file_name().unwrap_or_default().to_string_lossy().into_owned()
}

impl Session {
    pub fn current(&self) -> Option<&PathBuf> {
        self.tape.tracks.get(self.index)
    }

    fn track(&self, index: usize) -> Value {
        json!({"index": index, "path": self.tape.tracks.get(index), "position": 0})
    }

    /// Put a tape in the deck, discarding whatever an earlier archive left in scratch space.
    fn insert_tape(&mut self, tape: Tape, scratch: Option<PathBuf>) {
        if let Some(old) = std::mem::replace(&mut self.scratch, scratch) {
            let _ = std::fs::remove_dir_all(old);
        }
        (self.tape, self.index, self.dirty) = (tape, 0, false);
    }

    fn start(
        &mut self,
        label: String,
        total: u64,
        work: impl FnOnce(&AtomicU64) -> Result<Outcome, String> + Send + 'static,
    ) {
        let done = Arc::new(AtomicU64::new(0));
        let progress = done.clone();
        self.job = Some(Job { label, total, done, handle: std::thread::spawn(move || work(&progress)) });
    }

    fn import(&mut self, archive: PathBuf) -> Result<Value, String> {
        let into = tape::scratch(&tape::scratch_root(), &archive)?;
        let (label, total) = (format!("Loading {}", file_name(&archive)), tape::archive_size(&archive));
        self.start(label, total, move |done| {
            tape::extract(&archive, &into, done).map(|tape| Outcome::Imported(tape, into))
        });
        Ok(json!({"job": true}))
    }

    /// Open `paths` as the new tape, or with `append` add their audio to the current one.
    pub fn load(&mut self, request: &Value) -> Result<Value, String> {
        let paths = paths(request);
        let audio: Vec<PathBuf> =
            paths.iter().filter(|p| matches!(tape::kind(p), Kind::Audio | Kind::Other)).cloned().collect();
        if request["append"].as_bool().unwrap_or(false) {
            self.dirty |= !audio.is_empty();
            self.tape.tracks.extend(audio);
            return Ok(json!({"job": false}));
        }
        match paths.first().map(|first| (tape::kind(first), first)) {
            Some((Kind::Archive, archive)) => return self.import(archive.clone()),
            Some((Kind::Index, index)) => self.insert_tape(tape::read_index(index)?, None),
            _ if audio.is_empty() => return Err("Nothing to play there".into()),
            _ => self.insert_tape(Tape { tracks: audio, ..Tape::default() }, None),
        }
        Ok(json!({"job": false}))
    }

    pub fn export(&mut self, request: &Value) -> Result<Value, String> {
        let dest = PathBuf::from(request["dest"].as_str().ok_or("missing destination")?);
        let dest = if tape::kind(&dest) == Kind::Index { dest } else { dest.with_extension("tape") };
        let (tape, label) = (self.tape.clone(), format!("Saving {}", file_name(&dest)));
        self.start(label, tape::export_size(&tape), move |done| {
            tape::export(&tape, &dest, done).map(|()| Outcome::Exported(dest))
        });
        Ok(json!({"job": true}))
    }

    fn finish(&mut self, outcome: Result<Outcome, String>) -> Value {
        match outcome {
            Ok(Outcome::Imported(tape, scratch)) => {
                self.insert_tape(tape, Some(scratch));
                json!({"active": false, "loaded": true, "notice": ""})
            }
            Ok(Outcome::Exported(dest)) => {
                self.dirty = false;
                json!({"active": false, "loaded": false, "notice": format!("Saved {}", file_name(&dest))})
            }
            Err(error) => json!({"active": false, "loaded": false, "notice": error}),
        }
    }

    /// Progress of the background job; the call that finds it finished also applies its result.
    pub fn poll(&mut self) -> Value {
        match self.job.take() {
            None => json!({"active": false, "loaded": false, "notice": ""}),
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

    fn brief(&mut self, path: &Path) -> Value {
        let (title, artist, seconds) =
            self.briefs.entry(path.to_owned()).or_insert_with(|| insert::brief(path)).clone();
        json!({"path": path, "file": file_name(path), "title": title, "artist": artist, "seconds": seconds})
    }

    pub fn describe(&mut self) -> Value {
        let tracks: Vec<Value> = self.tape.tracks.clone().iter().map(|track| self.brief(track)).collect();
        let seconds: u64 = tracks.iter().filter_map(|t| t["seconds"].as_u64()).sum();
        let mixtape = tracks.len() > 1 || !self.tape.name.is_empty() || self.tape.cover.is_some();
        json!({"name": self.tape.name, "cover": self.tape.cover, "tracks": tracks, "index": self.index,
            "seconds": seconds, "dirty": self.dirty, "mixtape": mixtape})
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
        self.tape.cover = Some(cover);
        Ok(false)
    }

    /// Change the tape. The reply says whether the track that is up was taken away.
    pub fn edit(&mut self, request: &Value) -> Result<Value, String> {
        let number = |key: &str| request[key].as_u64().unwrap_or(u64::MAX) as usize;
        let replaced = match request["action"].as_str().unwrap_or("") {
            "remove" => self.remove(number("index"))?,
            "move" => self.reorder(number("from"), number("to"))?,
            "cover" => self.set_cover(request["path"].as_str().unwrap_or(""))?,
            "name" => {
                self.tape.name = request["name"].as_str().unwrap_or("").trim().to_owned();
                false
            }
            _ => return Err("unknown tape edit".into()),
        };
        self.dirty = true;
        Ok(json!({"replaced": replaced, "path": self.current()}))
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

    pub fn track_path(&self, request: &Value) -> Option<&PathBuf> {
        request["index"].as_u64().map_or(self.current(), |index| self.tape.tracks.get(index as usize))
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(scratch) = self.scratch.take() {
            let _ = std::fs::remove_dir_all(scratch);
        }
    }
}

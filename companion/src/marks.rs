//! Which chunks of a SavedVariables file an export has read, kept in `state.json`.
//!
//! The addon numbers chunks from `seq`, and `seq` starts again at 0 whenever the saved table is
//! gone (a deleted WTF folder, a file the game dropped for its size, a corrupted file). A mark
//! of "the highest `q` read" alone then skips every new chunk. So each stream also keeps the
//! highest start time `t` of its chunks, and a chunk at or below the mark counts as new when it
//! is later than that. Both come from every chunk an export judged, read or already held and
//! never one that would not decode, so a clock that stepped back between two chunks cannot pull
//! the mark under a chunk already read.
//!
//! A chunk with a `q` the mark holds and a `t` above the `t` of the highest-`q` chunk could still
//! belong to a store that started over, when the app did not run until the new `seq` passed the old
//! mark, or when one chunk written with the clock ahead keeps the highest `t` high. The export
//! then looks in its archive for that chunk before it counts it as read.
//!
//! Keys per stream, `key` being the SavedVariables path (the addon stream adds `#addons`):
//! `key` is the highest `q`, `key#t` the highest `t`, `key#qt` the `t` of the chunk with the highest
//! `q`, `key#restart` the `t` of the first chunk read after the store started over, `key#since` the
//! chunks read since, `key#bad` the chunks that would not decode at the last export. A `state.json`
//! that has only `key`, or `key` and `key#t`, is read as it was and gains the rest as it goes.

use crate::export::Chunk;
use chrono::{Datelike, Local, TimeZone};
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::BTreeMap;

pub(crate) type Marks = BTreeMap<String, u64>;

const T: &str = "#t";
const QT: &str = "#qt";
const RESTART: &str = "#restart";
const SINCE: &str = "#since";
const BAD: &str = "#bad";
const ADDONS: &str = "#addons";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Seen {
    Read,
    New,
    Restarted,
}

pub(crate) struct Cursor {
    key: String,
    q: u64,
    t: Option<i64>,
    q_t: Option<i64>,
    seq: Option<u64>,
    highest_q: Option<u64>,
    highest_t: Option<i64>,
    highest_q_t: Option<i64>,
    restarted_at: Option<i64>,
    after_restart: u64,
    read: u64,
    unreadable: u64,
}

impl Cursor {
    pub(crate) fn new(marks: &Marks, key: &str, seq: Option<u64>) -> Self {
        Self {
            key: key.to_string(),
            q: marks.get(key).copied().unwrap_or(0),
            t: marks.get(&format!("{key}{T}")).map(|t| *t as i64),
            q_t: marks.get(&format!("{key}{QT}")).map(|t| *t as i64),
            seq,
            highest_q: None,
            highest_t: None,
            highest_q_t: None,
            restarted_at: None,
            after_restart: 0,
            read: 0,
            unreadable: 0,
        }
    }

    pub(crate) fn judge(&self, ch: &Chunk, archive_holds_chunk: impl FnOnce() -> bool) -> Seen {
        if ch.q > self.q {
            return Seen::New;
        }
        if self.seq.is_some_and(|seq| seq < self.q) {
            return Seen::Restarted;
        }
        match self.t {
            Some(t) if ch.t > t => Seen::Restarted,
            Some(_) if self.q_t.is_some_and(|q_t| ch.t <= q_t) => Seen::Read,
            _ if archive_holds_chunk() => Seen::Read,
            _ => Seen::Restarted,
        }
    }

    fn judged(&mut self, ch: &Chunk) {
        match Some(ch.q).cmp(&self.highest_q) {
            Ordering::Greater => {
                self.highest_q = Some(ch.q);
                self.highest_q_t = Some(ch.t);
            }
            Ordering::Equal => self.highest_q_t = self.highest_q_t.min(Some(ch.t)),
            Ordering::Less => {}
        }
        self.highest_t = self.highest_t.max(Some(ch.t));
    }

    pub(crate) fn skipped(&mut self, ch: &Chunk) {
        self.judged(ch);
    }

    pub(crate) fn took(&mut self, ch: &Chunk, seen: Seen) {
        self.judged(ch);
        self.read += 1;
        if seen == Seen::Restarted && self.restarted_at.is_none() {
            self.restarted_at = Some(ch.t);
        }
        if self.restarted_at.is_some() {
            self.after_restart += 1;
        }
    }

    pub(crate) fn unreadable(&mut self) {
        self.unreadable += 1;
    }

    pub(crate) fn restarted_at(&self) -> Option<(i64, u64)> {
        self.restarted_at.map(|t| (t, self.after_restart))
    }

    pub(crate) fn unreadable_count(&self) -> u64 {
        self.unreadable
    }

    pub(crate) fn finish(self, marks: &mut Marks) {
        let key = &self.key;
        if let (Some(q), Some(t), Some(q_t)) = (self.highest_q, self.highest_t, self.highest_q_t) {
            marks.insert(key.clone(), q);
            marks.insert(format!("{key}{T}"), t as u64);
            marks.insert(format!("{key}{QT}"), q_t as u64);
        }
        if let Some(at) = self.restarted_at {
            marks.insert(format!("{key}{RESTART}"), at as u64);
            marks.insert(format!("{key}{SINCE}"), self.after_restart);
        } else if self.read > 0
            && let Some(since) = marks.get_mut(&format!("{key}{SINCE}"))
        {
            *since += self.read;
        }
        let bad = format!("{key}{BAD}");
        if self.unreadable > 0 {
            marks.insert(bad, self.unreadable);
        } else {
            marks.remove(&bad);
        }
    }
}

/// `Nov 22`, with the year when it is not this one.
pub fn short_day(epoch: i64) -> String {
    let Some(day) = Local.timestamp_opt(epoch, 0).single() else {
        return String::new();
    };
    let this_year = Local::now().year();
    let month = day.format("%b");
    if day.year() == this_year {
        format!("{month} {}", day.day())
    } else {
        format!("{month} {} {}", day.day(), day.year())
    }
}

pub fn restart_line(epoch: i64, since: u64, addons: bool) -> String {
    format!(
        "The game's Scarlet {} started over on {}; {since} {} read since.",
        if addons { "addon store" } else { "store" },
        short_day(epoch),
        if since == 1 {
            "recording"
        } else {
            "recordings"
        }
    )
}

pub fn unreadable_line(count: u64) -> String {
    format!(
        "{count} {} in SavedVariables could not be read.",
        if count == 1 {
            "recording"
        } else {
            "recordings"
        }
    )
}

/// One SavedVariables file whose store started over, as `state.json` last recorded it.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Restart {
    pub account: String,
    pub addons: bool,
    pub at: i64,
    pub read_since: u64,
}

/// What the last exports saw of the game's saved store, for the Data page.
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct StoreNotes {
    pub restarts: Vec<Restart>,
    pub unreadable: u64,
    pub files: u64,
}

fn account_of(key: &str) -> String {
    let path = key.strip_suffix(ADDONS).unwrap_or(key);
    let mut parts = path.rsplit(['\\', '/']);
    parts.next();
    parts.next();
    parts.next().unwrap_or("").to_string()
}

pub(crate) fn notes(marks: &Marks) -> StoreNotes {
    let mut out = StoreNotes::default();
    for (k, at) in marks {
        if let Some(key) = k.strip_suffix(RESTART) {
            out.restarts.push(Restart {
                account: account_of(key),
                addons: key.ends_with(ADDONS),
                at: *at as i64,
                read_since: marks.get(&format!("{key}{SINCE}")).copied().unwrap_or(0),
            });
        } else if k.ends_with(BAD) {
            out.unreadable += *at;
        }
    }
    out.restarts
        .sort_by(|a, b| (a.addons, &a.account).cmp(&(b.addons, &b.account)));
    out.files = marks.keys().filter(|k| k.ends_with("Scarlet.lua")).count() as u64;
    out
}

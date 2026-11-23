//! What the sessions list calls a session: one WoW run, one local day, or a stretch the player marked by hand.

use crate::store::Store;
use chrono::{Local, NaiveDate, TimeZone};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Two store sessions closer than this belong to one WoW run: a /reload or a character switch leaves a gap of seconds.
pub const RUN_GAP_MS: i64 = 5 * 60_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionMode {
    #[default]
    Auto,
    Day,
    Manual,
}

/// A stretch of time the player started and stopped in the app, epoch ms. `to` is None while it is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mark {
    pub from: i64,
    pub to: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Part {
    pub ch: u32,
    pub start: i64,
    pub end: i64,
}

#[derive(Clone, Debug)]
pub struct Group {
    pub from: i64,
    pub to: i64,
    pub live: bool,
    pub parts: Vec<Part>,
}

impl Group {
    /// The characters in the group, the one with the most time first.
    pub fn chars(&self) -> Vec<u32> {
        let mut time: Vec<(u32, i64)> = Vec::new();
        for p in &self.parts {
            match time.iter_mut().find(|t| t.0 == p.ch) {
                Some(t) => t.1 += p.end - p.start,
                None => time.push((p.ch, p.end - p.start)),
            }
        }
        time.sort_by_key(|t| std::cmp::Reverse(t.1));
        time.into_iter().map(|t| t.0).collect()
    }

    pub fn spans_of(&self, ch: u32) -> Vec<(i64, i64)> {
        self.parts
            .iter()
            .filter(|p| p.ch == ch)
            .map(|p| (p.start, p.end))
            .collect()
    }

    /// Seconds with a client recording, overlaps counted once.
    pub fn played(&self) -> i64 {
        let spans: Vec<(i64, i64)> = self.parts.iter().map(|p| (p.start, p.end)).collect();
        scarlet::recap::union_spans(&spans)
            .iter()
            .map(|s| (s.1 - s.0) / 1000)
            .sum()
    }
}

fn parts_of(st: &Store, ch: Option<u32>) -> Vec<Part> {
    let mut parts: Vec<Part> = st
        .sessions
        .iter()
        .filter(|s| ch.is_none_or(|c| s.ch == c) && s.end >= s.start)
        .map(|s| Part {
            ch: s.ch,
            start: s.start,
            end: s.end,
        })
        .collect();
    parts.sort_by_key(|p| (p.start, p.end, p.ch));
    parts.dedup();
    parts
}

fn auto(parts: Vec<Part>) -> Vec<Group> {
    let mut out: Vec<Group> = Vec::new();
    for p in parts {
        match out.last_mut() {
            Some(g) if p.start <= g.to + RUN_GAP_MS => {
                g.to = g.to.max(p.end);
                g.parts.push(p);
            }
            _ => out.push(Group {
                from: p.start,
                to: p.end,
                live: false,
                parts: vec![p],
            }),
        }
    }
    out
}

fn local_day(ms: i64) -> NaiveDate {
    Local
        .timestamp_millis_opt(ms)
        .single()
        .map_or(NaiveDate::MIN, |t| t.date_naive())
}

fn by_day(parts: Vec<Part>) -> Vec<Group> {
    let mut days: BTreeMap<NaiveDate, Vec<Part>> = BTreeMap::new();
    for p in parts {
        days.entry(local_day(p.start)).or_default().push(p);
    }
    days.into_values()
        .map(|parts| Group {
            from: parts.iter().map(|p| p.start).min().unwrap_or(0),
            to: parts.iter().map(|p| p.end).max().unwrap_or(0),
            live: false,
            parts,
        })
        .collect()
}

fn marked(parts: &[Part], marks: &[Mark], now: i64) -> Vec<Group> {
    marks
        .iter()
        .filter_map(|m| {
            let to = m.to.unwrap_or(now);
            let mut inside: Vec<Part> = parts
                .iter()
                .filter(|p| p.start <= to && p.end >= m.from)
                .map(|p| Part {
                    ch: p.ch,
                    start: p.start.max(m.from),
                    end: p.end.min(to),
                })
                .collect();
            inside.sort_by_key(|p| (p.start, p.end, p.ch));
            inside.dedup();
            let live = m.to.is_none();
            (live || !inside.is_empty()).then_some(Group {
                from: m.from,
                to,
                live,
                parts: inside,
            })
        })
        .collect()
}

/// The sessions of the list for `mode`, newest first. `ch` limits them to one character. Each group's parts are the
/// store sessions inside it, cut to it, an exact duplicate counted once.
pub fn groups(
    st: &Store,
    mode: SessionMode,
    marks: &[Mark],
    ch: Option<u32>,
    now: i64,
) -> Vec<Group> {
    let parts = parts_of(st, ch);
    let mut out = match mode {
        SessionMode::Auto => auto(parts),
        SessionMode::Day => by_day(parts),
        SessionMode::Manual => marked(&parts, marks, now),
    };
    out.sort_by_key(|g| std::cmp::Reverse((g.from, g.to)));
    out
}

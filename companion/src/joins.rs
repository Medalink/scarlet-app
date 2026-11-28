//! The loads the addon joined into one session. A `/reload` writes a session of its own, and the addon marks the rows of
//! the loads it joined with `o`, the key of the session's first load (`ScarletDB.chars[name].ov.sess`). The addon's ring
//! of rows is short, so each export keeps what it read in `joins.json` beside the character's overview, and feats count
//! sessions the way the addon does.

use crate::recap::Tally;
use std::collections::BTreeMap;
use std::path::Path;

pub const FILE: &str = "joins.json";

/// Two keys this close are the same session (the addon's own rule).
const SAME_KEY_SECS: i64 = 3;

/// Row key to the key of the session's first load, in epoch seconds.
#[derive(Default, PartialEq, Eq, Clone, Debug)]
pub struct Joins(BTreeMap<i64, i64>);

impl Joins {
    pub fn read(dir: &Path) -> Joins {
        let rows = std::fs::read_to_string(dir.join(FILE))
            .ok()
            .and_then(|text| serde_json::from_str::<BTreeMap<i64, i64>>(&text).ok())
            .unwrap_or_default();
        Joins(rows)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Adds rows `(t, o)` and says whether that changed anything.
    pub fn add(&mut self, rows: impl IntoIterator<Item = (i64, i64)>) -> bool {
        let before = self.0.len();
        let mut changed = false;
        for (t, o) in rows {
            if self.0.insert(t, o) != Some(o) {
                changed = true;
            }
        }
        changed || self.0.len() != before
    }

    pub fn text(&self) -> String {
        serde_json::to_string(&self.0).unwrap_or_else(|_| "{}".into())
    }

    fn origin_of(&self, start: i64) -> Option<i64> {
        self.0
            .range(start - SAME_KEY_SECS..=start + SAME_KEY_SECS)
            .min_by_key(|(t, _)| (**t - start).abs())
            .map(|(_, o)| *o)
    }

    /// Gives each load the session it belongs to: the key of the session's first load, else its own start.
    pub fn apply(&self, tallies: &mut [Tally]) {
        for t in tallies {
            t.origin = self.origin_of(t.start).unwrap_or(t.start);
        }
    }
}

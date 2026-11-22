//! ForeverDungeonJournal's run log, read into `fdj` run records of the counts archive (Scarlet's
//! FORMAT.md and the website's DESIGN.md, "Addon counts"). Only a closed run is read, one record each and
//! never joined with another; its kills, experience, money, deaths, party and loot rolls are not.

use crate::counts::{self, Record};
use crate::fishing::int;
use crate::lua::Lua;
use std::collections::BTreeMap;

pub const FILE: &str = "ForeverDungeonJournal.lua";
pub const TABLE: &str = "ForeverDungeonJournalCharacterDB";

/// One `run` record for every run of `runLog.runs` whose `closed` is true (the addon sets it when it
/// ends a run), with the run's `id` and `created` as its store. A run without those, `lastSeen` and
/// `map` is not read.
pub fn read(db: &Lua) -> Vec<Record> {
    let runs = db
        .get("runLog")
        .and_then(|log| log.get("runs"))
        .and_then(Lua::items)
        .unwrap_or(&[]);
    runs.iter()
        .filter_map(|(_, run)| {
            if !matches!(run.get("closed"), Some(Lua::Bool(true))) {
                return None;
            }
            let (id, created, last, map) = (
                int(run, "id")?,
                int(run, "created")?,
                int(run, "lastSeen")?,
                int(run, "map")?,
            );
            Some(Record {
                origin: counts::FDJ.to_string(),
                store: format!("{id}:{created}"),
                scope: counts::CHARACTER.to_string(),
                window: counts::SESSION.to_string(),
                kind: counts::RUN.to_string(),
                dims: format!("instance={map}"),
                from: Some(created),
                to: last.max(created),
                fields: BTreeMap::from([("runs".to_string(), 1)]),
            })
        })
        .collect()
}

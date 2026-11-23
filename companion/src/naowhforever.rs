//! NaowhForever's journal boss kills, read into `nf` boss records of the counts archive (Scarlet's
//! FORMAT.md and the website's DESIGN.md, "Addon counts"). Only the time of each kept kill is read, with
//! its encounter and the character the addon's `charGuid` names for the GUID. The undated kills past the
//! addon's cap, fight times, records, rare kills, party and loot are not, and no GUID is archived.

use crate::counts::{self, Record};
use crate::lua::Lua;
use std::collections::{BTreeMap, BTreeSet};

pub const FILE: &str = "NaowhForever.lua";
pub const TABLE: &str = "NaowhForeverDB";

fn character_of<'a>(key: &str, exported: &'a [String]) -> Option<&'a String> {
    let mut fit = exported.iter().filter(|name| {
        name.split_once('-')
            .is_some_and(|(first, last)| key.starts_with(&format!("{first} {last}-")))
    });
    let first = fit.next()?;
    fit.next().is_none().then_some(first)
}

/// One `boss` record for every kill time of `journalKills[<GUID>][<encounterID>].at`, with its
/// character. `exported` is the exported characters of the file's game account. A GUID that no
/// `charGuid` entry of an exported character names, or whose entries name two exported characters, is not read.
pub fn read(db: &Lua, exported: &[String]) -> Vec<(String, Vec<Record>)> {
    let (Some(guids), Some(kills)) = (
        db.get("charGuid"),
        db.get("account").and_then(|a| a.get("journalKills")),
    ) else {
        return Vec::new();
    };
    let mut by_name: BTreeMap<String, Vec<Record>> = BTreeMap::new();
    for (guid, bosses) in kills.items().unwrap_or(&[]) {
        let names: BTreeSet<&String> = guids
            .items()
            .unwrap_or(&[])
            .iter()
            .filter(|(_, g)| g.as_str() == Some(guid))
            .filter_map(|(key, _)| character_of(key, exported))
            .collect();
        let (Some(name), 1) = (names.first(), names.len()) else {
            continue;
        };
        for (encounter, record) in bosses.items().unwrap_or(&[]) {
            let Some(encounter) = encounter
                .parse::<i64>()
                .ok()
                .filter(|id| (1..=i64::from(i32::MAX)).contains(id))
            else {
                continue;
            };
            for (_, at) in record.get("at").and_then(Lua::items).unwrap_or(&[]) {
                let Some(at) = at.as_num().filter(|n| n.is_finite() && *n > 0.0) else {
                    continue;
                };
                let at = at.round() as i64;
                by_name.entry((*name).clone()).or_default().push(Record {
                    origin: counts::NF.to_string(),
                    store: String::new(),
                    scope: counts::CHARACTER.to_string(),
                    window: counts::EVENT.to_string(),
                    kind: counts::BOSS.to_string(),
                    dims: format!("encounter={encounter}"),
                    from: Some(at),
                    to: at,
                    fields: BTreeMap::from([("bosses".to_string(), 1)]),
                });
            }
        }
    }
    by_name.into_iter().collect()
}

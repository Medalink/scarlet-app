//! ForeverFishing's saved variables, read into `ff` session records of the counts archive (Scarlet's
//! FORMAT.md and the website's DESIGN.md, "Addon counts"). Only `sessionHistory` is read: the addon's
//! lifetime totals equal its saved sessions or count what is not fishing, and its journal, locations and
//! rod time are not counts.

use crate::counts::{self, Record};
use crate::fishing::{int, text};
use crate::lua::Lua;
use std::collections::BTreeMap;

pub const FILE: &str = "ForeverFishing.lua";
pub const TABLE: &str = "ForeverFishingDB";

/// The sessions one game character saved, with the name and realm of the latest of them. Both find
/// the exported character and go no further.
pub struct Saved {
    name: String,
    realm: String,
    records: Vec<Record>,
}

/// The saved sessions of every character in the file, grouped by the character's GUID, the key the
/// addon files them under.
pub fn read(db: &Lua) -> Vec<Saved> {
    let Some(history) = db.get("sessionHistory").and_then(Lua::items) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (_guid, sessions) in history {
        let mut saved = Saved {
            name: String::new(),
            realm: String::new(),
            records: Vec::new(),
        };
        let mut latest = i64::MIN;
        for (_, s) in sessions.items().unwrap_or(&[]) {
            let (Some(from), Some(to), Some(casts)) =
                (int(s, "startedAt"), int(s, "endedAt"), int(s, "casts"))
            else {
                continue;
            };
            let mut fields = BTreeMap::from([("casts".to_string(), casts)]);
            for name in ["successfulCasts", "catches"] {
                if let Some(n) = int(s, name) {
                    fields.insert(name.to_string(), n);
                }
            }
            if to >= latest {
                latest = to;
                saved.name = text(s, "character").unwrap_or_default().to_string();
                saved.realm = text(s, "realm").unwrap_or_default().to_string();
            }
            saved.records.push(Record {
                origin: counts::FOREVER_FISHING.to_string(),
                store: String::new(),
                scope: counts::CHARACTER.to_string(),
                window: counts::SESSION.to_string(),
                kind: counts::FISHING.to_string(),
                dims: String::new(),
                from: Some(from),
                to,
                fields,
            });
        }
        if !saved.records.is_empty() {
            out.push(saved);
        }
    }
    out
}

/// Hands each saved character's sessions to the exported character it is: the one whose name the
/// character's latest session carries, bare (`Name`) or whole (`Name-Surname`), with the realm
/// breaking a tie. Returns the records by exported name and the number of sessions that matched no
/// exported character or more than one.
pub fn assign(saved: Vec<Saved>, exported: &[String]) -> (BTreeMap<String, Vec<Record>>, usize) {
    let mut by_name: BTreeMap<String, Vec<Record>> = BTreeMap::new();
    let mut unmatched = 0;
    for s in saved {
        match find(&s.name, &s.realm, exported) {
            Some(name) => by_name.entry(name.clone()).or_default().extend(s.records),
            None => unmatched += s.records.len(),
        }
    }
    (by_name, unmatched)
}

fn find<'a>(name: &str, realm: &str, exported: &'a [String]) -> Option<&'a String> {
    let name = name.to_lowercase();
    if name.is_empty() {
        return None;
    }
    let hits: Vec<&String> = exported
        .iter()
        .filter(|c| {
            let c = c.to_lowercase();
            if name.contains('-') {
                c == name
            } else {
                c.split('-').next() == Some(name.as_str())
            }
        })
        .collect();
    let [first, rest @ ..] = hits.as_slice() else {
        return None;
    };
    if rest.is_empty() {
        return Some(first);
    }
    let realm = squash(realm);
    let mut same_realm = hits.iter().filter(|c| {
        !realm.is_empty() && c.split_once('-').is_some_and(|(_, r)| squash(r) == realm)
    });
    match (same_realm.next(), same_realm.next()) {
        (Some(one), None) => Some(one),
        _ => None,
    }
}

fn squash(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

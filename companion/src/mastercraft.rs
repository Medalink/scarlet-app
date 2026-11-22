//! Mastercraft's saved variables: the account's `mc` fishing, herb and ore totals for the counts archive
//! (Scarlet's FORMAT.md and the website's DESIGN.md, "Addon counts") and each character's last balance of every
//! local day for the imports archive (FORMAT.md, "Imported history"). The file also holds the names
//! of other players, which are never read, logged or archived.

use crate::counts::{self, Record};
use crate::fishing::int;
use crate::importarchive::Entry;
use crate::lua::Lua;
use chrono::NaiveDate;
use std::collections::BTreeMap;

pub const FILE: &str = "Mastercraft.lua";
pub const TABLE: &str = "MastercraftDB";

/// The lifetime fishing total of the whole game account: casts, catches and the items gained, as of
/// `modified`, the file's modification time in epoch seconds. `None` when the file holds no total or
/// the total is empty.
pub fn life(db: &Lua, modified: i64) -> Option<Record> {
    let life = db.get("fishing")?.get("life")?;
    let casts = int(life, "casts").unwrap_or(0);
    let catches = int(life, "catches").unwrap_or(0);
    let gained: i64 = life
        .get("gained")
        .and_then(Lua::items)
        .unwrap_or(&[])
        .iter()
        .filter_map(|(_, n)| n.as_num().filter(|n| n.is_finite()))
        .map(|n| n.round() as i64)
        .sum();
    if casts == 0 && catches == 0 && gained == 0 {
        return None;
    }
    let since = int(life, "since");
    Some(Record {
        origin: counts::MASTERCRAFT.to_string(),
        store: since.map(|s| s.to_string()).unwrap_or_default(),
        scope: counts::ACCOUNT.to_string(),
        window: counts::LIFE.to_string(),
        kind: counts::FISHING.to_string(),
        dims: String::new(),
        from: since,
        to: modified,
        fields: BTreeMap::from([
            ("casts".to_string(), casts),
            ("catches".to_string(), catches),
            ("gained".to_string(), gained),
        ]),
    })
}

/// The gathers `nodes[<mapID>][i].c` count, summed by profession over every map, as account `life`
/// claims of kind `herb` (Herbalism) and `ore` (Mining) as of `modified`. A node with no `c` is one
/// gather, as the addon reads it. A profession with no gathers gives no record, and `gmNodes` is not
/// read.
pub fn gathers(db: &Lua, modified: i64) -> Vec<Record> {
    let mut herb = 0;
    let mut ore = 0;
    for (_, map) in db.get("nodes").and_then(Lua::items).unwrap_or(&[]) {
        for (_, node) in map.items().unwrap_or(&[]) {
            let c = int(node, "c").unwrap_or(1).max(0);
            match node.get("p").and_then(Lua::as_str) {
                Some("Herbalism") => herb += c,
                Some("Mining") => ore += c,
                _ => {}
            }
        }
    }
    [(counts::HERB, herb), (counts::ORE, ore)]
        .into_iter()
        .filter(|(_, total)| *total > 0)
        .map(|(kind, total)| Record {
            origin: counts::MASTERCRAFT.to_string(),
            store: String::new(),
            scope: counts::ACCOUNT.to_string(),
            window: counts::LIFE.to_string(),
            kind: kind.to_string(),
            dims: String::new(),
            from: None,
            to: modified,
            fields: BTreeMap::from([("c".to_string(), total)]),
        })
        .collect()
}

fn plain(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn character_of<'a>(entry: &Lua, chars: &'a [String]) -> Option<&'a String> {
    let name = plain(entry.get("name")?.as_str()?);
    let surname = entry.get("surname").and_then(Lua::as_str).map(plain);
    let mut fit = chars.iter().filter(|c| {
        let (n, r) = c.split_once('-').unwrap_or((c, ""));
        plain(n) == name
            && surname
                .as_deref()
                .is_none_or(|w| w.is_empty() || w == plain(r))
    });
    let first = fit.next()?;
    fit.next().is_none().then_some(first)
}

/// Every day `history[<GUID>][<YYYY-MM-DD>].g` holds, each with its character. `folder` is every
/// character of the file's game account and `own` the exported ones. A GUID is matched to a
/// character through its `chars[<GUID>]` name, and one that fits none is skipped. The day `today`
/// and later ones are not over and are left out.
pub fn days(db: &Lua, folder: &[String], own: &[String], today: NaiveDate) -> Vec<(String, Entry)> {
    let mut out = Vec::new();
    let (Some(known), Some(history)) = (db.get("chars"), db.get("history")) else {
        return out;
    };
    for (guid, by_day) in history.items().unwrap_or(&[]) {
        let Some(name) = known
            .get(guid)
            .and_then(|e| character_of(e, folder))
            .filter(|n| own.contains(n))
        else {
            continue;
        };
        for (date, day) in by_day.items().unwrap_or(&[]) {
            let Some(copper) = day.get("g").and_then(Lua::as_num) else {
                continue;
            };
            let over = NaiveDate::parse_from_str(date, "%Y-%m-%d").is_ok_and(|d| d < today);
            if copper.is_finite() && copper >= 0.0 && over {
                out.push((
                    name.clone(),
                    Entry::McDay {
                        date: date.clone(),
                        copper: copper.round() as i64,
                    },
                ));
            }
        }
    }
    out
}

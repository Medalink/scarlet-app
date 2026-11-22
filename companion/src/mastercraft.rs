//! Mastercraft's saved variables, read into the account's `mc` fishing total of the counts archive
//! (Scarlet's FORMAT.md and the website's DESIGN.md, "Addon counts"). Only `fishing.life` is read. The
//! file also holds the names of other players, which are never read, logged or archived.

use crate::counts::{self, Record};
use crate::fishing::int;
use crate::lua::Lua;
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

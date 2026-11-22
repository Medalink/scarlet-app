//! RXPGuides' leveling tracker, read into `rxp` level records of the counts archive (Scarlet's FORMAT.md
//! and the website's DESIGN.md, "Addon counts"). Only finished levels are read, and only their kills and
//! quest experience: mob experience, deaths, group experience and the archive of earlier lives are not.

use crate::counts::{self, Record};
use crate::fishing::int;
use crate::lua::Lua;
use chrono::{Local, NaiveDate, TimeZone};
use std::collections::BTreeMap;

pub const FILE: &str = "RXPGuides.lua";
pub const TABLE: &str = "RXPCTrackingData";

fn calendar(stamp: &Lua) -> Option<i64> {
    let field = |key: &str| u32::try_from(int(stamp, key)?).ok();
    let date = NaiveDate::from_ymd_opt(
        i32::try_from(int(stamp, "year")?).ok()?,
        field("month")?,
        field("monthDay")?,
    )?
    .and_hms_opt(field("hour")?, field("minute")?, 0)?;
    let local = Local.from_local_datetime(&date);
    Some(local.earliest().or_else(|| local.latest())?.timestamp())
}

fn quest_xp(level: &Lua) -> BTreeMap<i64, i64> {
    let mut by_quest = BTreeMap::new();
    for (_, zone) in level.get("quests").and_then(Lua::items).unwrap_or(&[]) {
        for (id, xp) in zone.items().unwrap_or(&[]) {
            if let (Ok(id), Some(xp)) = (id.parse::<i64>(), xp.as_num().filter(|n| n.is_finite())) {
                *by_quest.entry(id).or_insert(0) += xp.round() as i64;
            }
        }
    }
    by_quest.retain(|_, xp| *xp > 0);
    by_quest
}

/// The kills and quest experience of every finished level of `<name> - PvE`, with `name` the
/// character's name without its realm. A level without a `dateFinished` is not read. The level's
/// start is `dateStarted`, or its end when RXPGuides began inside the level. Calendar fields are read
/// in this PC's time zone.
pub fn read(db: &Lua, name: &str) -> Vec<Record> {
    let key = format!("{} - PvE", name.split('-').next().unwrap_or(name));
    let Some(profile) = db.get("profiles").and_then(|p| p.get(&key)) else {
        return Vec::new();
    };
    let store = profile
        .get("trackedGuid")
        .and_then(Lua::as_str)
        .unwrap_or_default();
    let mut out = Vec::new();
    for (level, entry) in profile.get("levels").and_then(Lua::items).unwrap_or(&[]) {
        let Ok(level) = level.parse::<u32>() else {
            continue;
        };
        let Some(stamp) = entry.get("timestamp") else {
            continue;
        };
        let Some(to) = stamp.get("dateFinished").and_then(calendar) else {
            continue;
        };
        let from = stamp
            .get("dateStarted")
            .and_then(calendar)
            .filter(|f| *f <= to)
            .unwrap_or(to);
        let kills: i64 = entry
            .get("mobs")
            .and_then(Lua::items)
            .unwrap_or(&[])
            .iter()
            .filter_map(|(_, zone)| int(zone, "count"))
            .sum();
        let record = |kind: &str, dims: String, field: &str, amount: i64| Record {
            origin: counts::RXP.to_string(),
            store: store.to_string(),
            scope: counts::CHARACTER.to_string(),
            window: counts::LEVEL.to_string(),
            kind: kind.to_string(),
            dims,
            from: Some(from),
            to,
            fields: BTreeMap::from([(field.to_string(), amount)]),
        };
        out.push(record(
            counts::KILL,
            format!("level={level}"),
            "count",
            kills,
        ));
        for (quest, xp) in quest_xp(entry) {
            out.push(record(
                counts::XP,
                format!("level={level};quest={quest}"),
                "xp",
                xp,
            ));
        }
    }
    out
}

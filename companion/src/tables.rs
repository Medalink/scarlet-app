//! Relational CSV tables per character (`<Name-Surname>/tables/`), rebuilt in
//! full on each export: every event with its place and context tags, the
//! session list, name lines, the SC/WS numbers and the loot tables, per
//! docs/FORMAT.md "Companion output". The place, the tags and the loot rule
//! are derived here exactly as the "Context" section defines them.

use crate::archive::Event;
use crate::events;
use crate::loot::{self, LootBook};
use crate::talents;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

/// A WS/SC value as an integer: a plain number as is, or money written with
/// `|T...|t` coin icon escapes — amounts in order gold, silver, copper, the
/// icon after each — read into copper.
pub fn stat_number(v: &str) -> Option<i64> {
    if let Ok(n) = v.parse::<i64>() {
        return Some(n);
    }
    // The Statistics panel shows "--" for a statistic that has never counted.
    if v.trim() == "--" {
        return Some(0);
    }
    let mut total = 0i64;
    let mut pending = 0i64;
    let mut bare = false;
    let mut any = false;
    for tok in v.split_whitespace() {
        // The client writes "98|TInterface\MoneyFrame\UI-GoldIcon:0:0:2:0|t":
        // the amount and its coin icon in one token.
        let (amount, icon) = tok.split_at(tok.find("|T").unwrap_or(tok.len()));
        if !amount.is_empty() {
            // Large amounts may be grouped ("1,234" or "1.234").
            let digits: String = amount.chars().filter(|c| !matches!(c, ',' | '.')).collect();
            match digits.parse::<i64>() {
                Ok(n) => {
                    pending = n;
                    bare = true;
                }
                // An amount we cannot read beside a coin icon: no number rather than a wrong one.
                Err(_) if !icon.is_empty() => return None,
                Err(_) => continue,
            }
        }
        if !icon.is_empty() {
            let tok = icon;
            let mult = if tok.contains("GoldIcon") {
                10_000
            } else if tok.contains("SilverIcon") {
                100
            } else if tok.contains("CopperIcon") {
                1
            } else {
                continue;
            };
            total += pending * mult;
            pending = 0;
            bare = false;
            any = true;
        }
    }
    (any && !bare).then_some(total)
}

/// The value without its `|T...|t` escapes.
pub fn plain_value(v: &str) -> String {
    let mut out = String::new();
    let mut rest = v;
    while let Some(i) = rest.find("|T") {
        out.push_str(&rest[..i]);
        rest = match rest[i..].find("|t") {
            Some(j) => &rest[i + j + 2..],
            None => "",
        };
    }
    out.push_str(rest);
    out
}

/// One CSV line, RFC 4180 quoting: fields with comma, quote or newline are
/// wrapped in quotes and inner quotes are doubled.
pub(crate) fn csv(fields: &[String]) -> String {
    let mut line = String::new();
    for (i, f) in fields.iter().enumerate() {
        if i > 0 {
            line.push(',');
        }
        if f.contains(',') || f.contains('"') || f.contains('\n') || f.contains('\r') {
            line.push('"');
            line.push_str(&f.replace('"', "\"\""));
            line.push('"');
        } else {
            line.push_str(f);
        }
    }
    line.push('\n');
    line
}

fn field(e: &Event, i: usize) -> String {
    e.fields.get(i).cloned().unwrap_or_default()
}

struct SessionRec {
    start: i64,
    end: i64,
    ended: bool,
    build: String,
    class: String,
    race: String,
    level_start: String,
    level_end: String,
    events: u64,
}

/// Write the seven tables for one character's events (every payload, in time
/// order) into `dir`. `me` is the character's own Name-Surname.
pub fn write_char_tables(events: &[Event], me: &str, dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;

    // Pass 1: sessions, name lines, and W movement intervals. A W line is
    // written when the stretch ends, so its interval runs from its time
    // minus its seconds to its time.
    let mut sessions: Vec<SessionRec> = Vec::new();
    let mut names: Vec<(char, String, String)> = Vec::new();
    let mut seen: HashSet<(char, String, String)> = HashSet::new();
    let mut walks: Vec<(i64, i64, &'static str)> = Vec::new();
    let mut cur: Option<usize> = None;
    for e in events {
        match e.code.as_str() {
            "S" => {
                sessions.push(SessionRec {
                    start: e.ms,
                    end: e.ms,
                    ended: false,
                    build: field(e, 0),
                    class: field(e, 1),
                    race: field(e, 2),
                    level_start: field(e, 3),
                    level_end: field(e, 3),
                    events: 0,
                });
                cur = Some(sessions.len() - 1);
            }
            "ES" => {
                if let Some(i) = cur {
                    sessions[i].ended = true;
                }
                cur = None;
            }
            "L" => {
                if let Some(i) = cur {
                    sessions[i].level_end = field(e, 0);
                }
            }
            "W" => {
                let tag = match field(e, 0).as_str() {
                    "f" => "running",
                    "w" => "walking",
                    "m" => "mounted",
                    "s" => "swimming",
                    "a" => "flying",
                    _ => "",
                };
                if !tag.is_empty() {
                    let secs: i64 = field(e, 2).parse().unwrap_or(0);
                    walks.push((e.ms - secs * 1000, e.ms, tag));
                }
            }
            _ => {}
        }
        if let Some((kind, id, name)) = name_line(e)
            && seen.insert((kind, id.clone(), name.clone()))
        {
            names.push((kind, id, name));
        }
        if let Some(i) = cur {
            let s = &mut sessions[i];
            s.end = s.end.max(e.ms);
            s.events += 1;
        }
    }

    // Pass 2: every event row with the tags on at its moment. Movement tags
    // come from the intervals collected above, swept in start order with an
    // active list, since events and interval ends interleave.
    walks.sort_by_key(|w| w.0);
    let mut wi = 0usize;
    let mut active: Vec<(i64, i64, &'static str)> = Vec::new();

    let (mut combat, mut group, mut resting) = (false, false, false);
    let (mut afk, mut dead, mut camp) = (false, false, false);
    let mut ghost = false;
    let (mut profession, mut taxi, mut instance, mut boss) = (false, false, false, false);
    let (mut saved, mut trading) = (false, false);
    let (mut bg, mut arena) = (false, false);
    let mut cinematic = false;
    // Interaction types of the NPC and object windows open now (NW).
    let mut windows: HashSet<String> = HashSet::new();
    // Crafting runs from a CB to its CR/CS, or 10 s after the CB.
    let mut craft_until: Option<i64> = None;
    let (mut cur, mut sess_count) = (None::<usize>, 0usize);
    let mut z: Option<[String; 4]> = None; // map, instance, zone, subzone
    let mut p: Option<[String; 2]> = None; // x, y

    let mut book = LootBook::new(me);

    let mut ev_rows = String::from(
        "ms,session,code,f1,f2,f3,f4,f5,f6,f7,f8,f9,f10,f11,f12,map,instance,zone,subzone,x,y,tags,loot_rule,loot_threshold,group_size\n",
    );
    let mut stat_rows = String::from("ms,session,key,value,reason\n");
    let mut statistic_rows = String::from("ms,session,id,value,number\n");

    for e in events {
        book.apply(e.ms, &e.code, &e.fields);
        match e.code.as_str() {
            "S" => {
                combat = false;
                group = false;
                resting = false;
                afk = false;
                dead = false;
                ghost = false;
                camp = false;
                profession = false;
                taxi = false;
                instance = false;
                boss = false;
                saved = false;
                trading = false;
                bg = false;
                arena = false;
                cinematic = false;
                windows.clear();
                craft_until = None;
                cur = Some(sess_count);
                sess_count += 1;
            }
            "ES" => cur = None,
            "F" => combat = field(e, 0) == "1",
            "GP" => group = field(e, 0) == "1",
            "RS" => resting = field(e, 0) == "1",
            "AFK" => afk = field(e, 0) == "1",
            "D" => {
                dead = true;
                ghost = false;
            }
            "R" => ghost = true,
            "U" => {
                dead = false;
                ghost = false;
            }
            "CP" => camp = field(e, 0) == "1",
            "TS" => profession = field(e, 0) == "1",
            "CB" => craft_until = Some(e.ms + 10_000),
            "CR" | "CS" => craft_until = None,
            "T" => taxi = field(e, 0) == "1",
            "E" => boss = field(e, 0) == "1",
            "IL" => saved = field(e, 0) == "1",
            "TR" => trading = field(e, 0) == "1",
            "CV" => cinematic = field(e, 0) == "1",
            "NW" => {
                let kind = field(e, 1);
                if field(e, 0) == "1" {
                    windows.insert(kind);
                } else {
                    windows.remove(&kind);
                }
            }
            "Z" => {
                let it = field(e, 2);
                instance = !it.is_empty() && it != "none";
                bg = it == "pvp";
                arena = it == "arena";
                z = Some([field(e, 0), field(e, 1), field(e, 4), field(e, 5)]);
            }
            "P" => p = Some([field(e, 1), field(e, 2)]),
            _ => {}
        }

        // Movement intervals active at this moment.
        active.retain(|w| w.1 > e.ms);
        while wi < walks.len() && walks[wi].0 <= e.ms {
            if walks[wi].1 > e.ms {
                active.push(walks[wi]);
            }
            wi += 1;
        }

        let mut tags: Vec<&str> = Vec::new();
        if combat {
            tags.push("combat");
        }
        if group {
            tags.push("group");
        }
        if resting {
            tags.push("resting");
        }
        if afk {
            tags.push("afk");
        }
        if dead {
            tags.push("dead");
        }
        if ghost {
            tags.push("ghost");
        }
        if camp {
            tags.push("camp");
        }
        if profession {
            tags.push("profession");
        }
        if craft_until.is_some_and(|u| e.ms <= u) {
            tags.push("crafting");
        }
        if taxi {
            tags.push("taxi");
        }
        if instance {
            tags.push("instance");
        }
        if bg {
            tags.push("bg");
        }
        if arena {
            tags.push("arena");
        }
        if boss {
            tags.push("boss");
        }
        if saved {
            tags.push("saved");
        }
        if !windows.is_empty() {
            tags.push("window");
        }
        if trading {
            tags.push("trade");
        }
        if cinematic {
            tags.push("cinematic");
        }
        for w in &active {
            tags.push(w.2);
        }

        let sess = cur.map(|i| i.to_string()).unwrap_or_default();
        let mut row = Vec::with_capacity(25);
        row.push(e.ms.to_string());
        row.push(sess.clone());
        row.push(e.code.clone());
        for i in 0..11 {
            row.push(field(e, i));
        }
        // f13 and later are joined into f12 by tabs.
        row.push(
            e.fields
                .iter()
                .skip(11)
                .cloned()
                .collect::<Vec<_>>()
                .join("\t"),
        );
        match &z {
            Some([map, inst, zone, sub]) => {
                row.push(map.clone());
                row.push(inst.clone());
                row.push(zone.clone());
                row.push(sub.clone());
            }
            None => row.extend(["".into(), "".into(), "".into(), "".into()]),
        }
        match &p {
            Some([x, y]) => {
                row.push(x.clone());
                row.push(y.clone());
            }
            None => row.extend(["".into(), "".into()]),
        }
        row.push(tags.join("|"));
        let rule = book.rule();
        row.push(rule.method.clone());
        row.push(rule.threshold.clone());
        row.push(if rule.size > 0 {
            rule.size.to_string()
        } else {
            String::new()
        });
        ev_rows.push_str(&csv(&row));

        match e.code.as_str() {
            "SC" | "SX" => {
                stat_rows.push_str(&csv(&[
                    e.ms.to_string(),
                    sess,
                    field(e, 1),
                    field(e, 2),
                    field(e, 0),
                ]));
            }
            "WS" => {
                let num = stat_number(&field(e, 1))
                    .map(|n| n.to_string())
                    .unwrap_or_default();
                statistic_rows.push_str(&csv(&[
                    e.ms.to_string(),
                    sess,
                    field(e, 0),
                    field(e, 1),
                    num,
                ]));
            }
            _ => {}
        }
    }

    let mut session_rows = String::from(
        "session,start_ms,end_ms,ended,build,class,race,level_start,level_end,events\n",
    );
    for (i, s) in sessions.iter().enumerate() {
        session_rows.push_str(&csv(&[
            i.to_string(),
            s.start.to_string(),
            s.end.to_string(),
            if s.ended { "1".into() } else { "0".into() },
            s.build.clone(),
            s.class.clone(),
            s.race.clone(),
            s.level_start.clone(),
            s.level_end.clone(),
            s.events.to_string(),
        ]));
    }

    let mut name_rows = String::from("kind,id,name\n");
    for (kind, id, name) in &names {
        name_rows.push_str(&csv(&[kind.to_string(), id.clone(), name.clone()]));
    }

    let name_map: HashMap<String, String> = names
        .iter()
        .map(|(kind, id, name)| (events::name_key(*kind, id), name.clone()))
        .collect();
    fs::write(
        dir.join("drops.csv"),
        loot::drops_csv(&book.drop_rows(&name_map)),
    )?;
    fs::write(
        dir.join("attribution.csv"),
        loot::attribution_csv(&book.attribution_rows()),
    )?;
    fs::write(dir.join("events.csv"), ev_rows)?;
    fs::write(dir.join("sessions.csv"), session_rows)?;
    fs::write(dir.join("names.csv"), name_rows)?;
    fs::write(dir.join("stats.csv"), stat_rows)?;
    fs::write(dir.join("statistics.csv"), statistic_rows)?;
    fs::write(dir.join("talents.csv"), talents::from_events(events).csv())?;
    Ok(())
}

/// The name-defining codes with the kind of name each sets: N, SN, IN, CN,
/// QN, KN, WN, AN, GN, FN, BN, TN, TY.
fn name_line(e: &Event) -> Option<(char, String, String)> {
    let kind = events::name_kind(&e.code)?;
    let id = e.fields.first()?;
    let name = e.fields.get(1)?;
    Some((kind, id.clone(), name.clone()))
}

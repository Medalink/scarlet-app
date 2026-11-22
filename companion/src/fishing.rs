//! The Fishing Log's saved variables, read into the `J` lines of Scarlet's FORMAT.md ("The Fishing
//! Log"): the character's records, journal, sessions and bounties from its own file, and its game
//! account's shared record book, boards, anglers and surveys from the account's file. Every line is
//! a snapshot of what the files hold now, timed by what it describes.

use crate::counts;
use crate::gamedir::sorted_dirs;
use crate::lua::{self, Lua};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const FILE: &str = "TheFishingLog.lua";
const MAGIC: &str = "scarletfishing";
const DIR: &str = "fishing";

const DAY_S: i64 = 86_400;
const WEEK_S: i64 = 604_800;
const MONTH_S: i64 = 2_419_200;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Line {
    pub ms: i64,
    pub code: &'static str,
    pub fields: Vec<String>,
}

/// A game account folder's own file, if it has one, and the files of its characters that have
/// one, by folder name (`Name-Surname`).
pub struct Account {
    pub db: Option<PathBuf>,
    pub chars: Vec<(String, PathBuf)>,
}

/// Every game account under `game` that has a file of The Fishing Log, its own or a character's.
pub fn find(game: &Path) -> Vec<Account> {
    let mut out = Vec::new();
    for acc in sorted_dirs(&game.join("WTF").join("Account")) {
        let db = Some(acc.join("SavedVariables").join(FILE)).filter(|p| p.is_file());
        let mut chars = Vec::new();
        for realm in sorted_dirs(&acc) {
            for ch in sorted_dirs(&realm) {
                let p = ch.join("SavedVariables").join(FILE);
                if let (true, Some(name)) = (p.is_file(), ch.file_name()) {
                    chars.push((name.to_string_lossy().into_owned(), p));
                }
            }
        }
        if db.is_some() || !chars.is_empty() {
            out.push(Account { db, chars });
        }
    }
    out
}

fn get<'a>(v: &'a Lua, key: &str) -> Option<&'a Lua> {
    v.get(key)
}

fn entries(v: Option<&Lua>) -> &[(String, Lua)] {
    v.and_then(Lua::items).unwrap_or(&[])
}

fn num(v: &Lua, key: &str) -> Option<f64> {
    get(v, key)?.as_num().filter(|n| n.is_finite())
}

pub(crate) fn int(v: &Lua, key: &str) -> Option<i64> {
    num(v, key).map(|n| n.round() as i64)
}

pub(crate) fn text<'a>(v: &'a Lua, key: &str) -> Option<&'a str> {
    get(v, key)?.as_str()
}

fn yes(v: &Lua, key: &str) -> Option<bool> {
    match get(v, key)? {
        Lua::Bool(b) => Some(*b),
        other => other.as_num().map(|n| n != 0.0),
    }
}

fn clean(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c == '\t' || c == '\n' || c == '\r' {
                ' '
            } else {
                c
            }
        })
        .collect()
}

fn f_int(n: Option<i64>) -> String {
    n.map(|n| n.to_string()).unwrap_or_default()
}

fn f_str(s: Option<&str>) -> String {
    s.map(clean).unwrap_or_default()
}

fn f_weight(n: Option<f64>) -> String {
    let Some(n) = n else {
        return String::new();
    };
    let s = format!("{:.2}", (n * 100.0).round() / 100.0);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

fn f_bit(b: Option<bool>) -> String {
    b.map(|b| if b { "1" } else { "0" }.to_string())
        .unwrap_or_default()
}

fn f_coord(n: Option<f64>) -> String {
    f_int(n.map(|n| (n * 10_000.0).round() as i64))
}

fn period_start(period: &str, index: i64) -> i64 {
    let span = if period.starts_with("week") {
        WEEK_S
    } else if period.starts_with("month") {
        MONTH_S
    } else {
        DAY_S
    };
    index * span
}

fn first_last_without_realm(raw: &str) -> String {
    let raw = raw.trim();
    let base = if raw.contains(' ') {
        raw.split('-').next().unwrap_or(raw)
    } else {
        raw
    };
    clean(&base.split_whitespace().collect::<Vec<_>>().join("-"))
        .trim_matches('-')
        .to_string()
}

fn angler_key_like_fishing_log(name: &str) -> String {
    name.chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .flat_map(char::to_lowercase)
        .collect()
}

#[derive(Default)]
struct Lines {
    out: Vec<Line>,
}

impl Lines {
    fn push(&mut self, at_s: i64, code: &'static str, fields: Vec<String>) {
        self.out.push(Line {
            ms: at_s * 1000,
            code,
            fields,
        });
    }
}

/// Fish names to item IDs, from the account's `fishNames` and the character's records, for the
/// sessions, which name their biggest fish only by name.
pub fn fish_ids(account: Option<&Lua>, character: Option<&Lua>) -> BTreeMap<String, i64> {
    let mut ids = BTreeMap::new();
    for (id, name) in entries(account.and_then(|a| get(a, "fishNames"))) {
        if let (Ok(id), Some(name)) = (id.parse::<i64>(), name.as_str()) {
            ids.insert(name.to_string(), id);
        }
    }
    for (_, spot) in entries(character.and_then(|c| get(c, "records"))) {
        for (id, rec) in entries(Some(spot)) {
            if let (Ok(id), Some(name)) = (id.parse::<i64>(), text(rec, "name")) {
                ids.entry(name.to_string()).or_insert(id);
            }
        }
    }
    ids
}

/// The `J` lines of one character's file (`TheFishingLogChar`).
pub fn read_char(db: &Lua, ids: &BTreeMap<String, i64>) -> Vec<Line> {
    let mut l = Lines::default();
    let empty = Lua::Table(Vec::new());
    let profile = get(db, "profile").unwrap_or(&empty);
    let updated = int(profile, "updated").unwrap_or(0);

    let latest = get(profile, "latestSpecimen").unwrap_or(&empty);
    l.push(
        updated,
        "JP",
        vec![
            f_int(int(profile, "xp")),
            f_int(int(profile, "season")),
            f_int(int(profile, "seasonXP")),
            f_int(int(profile, "value")),
            f_int(int(latest, "id")),
            f_weight(num(latest, "weight")),
            f_str(text(latest, "spot")),
            f_int(int(latest, "at")),
        ],
    );

    for (spot, fish) in entries(get(db, "records")) {
        for (item, r) in entries(Some(fish)) {
            l.push(
                int(r, "when").unwrap_or(updated),
                "JR",
                vec![
                    clean(spot),
                    clean(item),
                    f_weight(num(r, "weight")),
                    f_bit(yes(r, "specimen")),
                ],
            );
        }
    }

    for (item, j) in entries(get(profile, "journal")) {
        let first = get(j, "firstCatch").unwrap_or(&empty);
        l.push(
            int(first, "at").unwrap_or(updated),
            "JJ",
            vec![
                clean(item),
                f_int(int(j, "c")),
                f_weight(num(j, "b")),
                f_weight(num(j, "s")),
                f_str(text(first, "key")),
                f_str(text(first, "name")),
                f_int(int(first, "map")),
                f_coord(num(first, "x")),
                f_coord(num(first, "y")),
            ],
        );
    }

    for (item, zones) in entries(get(db, "caughtIn")) {
        for (zone, z) in entries(Some(zones)) {
            l.push(
                updated,
                "JZ",
                vec![
                    clean(item),
                    clean(zone),
                    f_int(int(z, "c")),
                    f_weight(num(z, "b")),
                    f_weight(num(z, "s")),
                    f_str(text(z, "spot")),
                ],
            );
        }
    }

    let biggest = |v: &Lua| -> (String, String) {
        let name = text(v, "biggestName");
        (f_int(name.and_then(|n| ids.get(n).copied())), f_str(name))
    };
    for (_, h) in entries(get(db, "history")) {
        let at = int(h, "when").unwrap_or(updated);
        let out = get(h, "outcomes").unwrap_or(&empty);
        let recs = get(h, "recordStats").unwrap_or(&empty);
        let (big_id, big_name) = biggest(h);
        l.push(
            at,
            "JH",
            vec![
                f_str(text(h, "zone")),
                f_str(text(h, "sub")),
                f_int(int(h, "duration").or_else(|| int(h, "elapsed"))),
                f_int(int(h, "count")),
                f_weight(num(h, "total")),
                f_weight(num(h, "biggest")),
                big_id,
                big_name,
                f_int(int(h, "specimens")),
                f_int(int(h, "value")),
                f_bit(yes(h, "solo")),
                f_int(int(out, "casts")),
                f_int(int(out, "landed")),
                f_int(int(out, "escaped")),
                f_int(int(out, "early")),
                f_int(int(out, "cancelled")),
                f_int(int(out, "unknown")),
                f_int(int(recs, "all")),
                f_int(int(recs, "season")),
            ],
        );
        for (_, w) in entries(get(h, "spots")) {
            let (big_id, big_name) = biggest(w);
            l.push(
                at,
                "JW",
                vec![
                    f_str(text(w, "name")),
                    f_int(int(w, "seconds")),
                    f_int(int(w, "count")),
                    f_weight(num(w, "total")),
                    f_weight(num(w, "biggest")),
                    big_id,
                    big_name,
                    f_int(int(w, "junk")),
                    f_int(int(w, "specimens")),
                    f_int(int(w, "value")),
                ],
            );
        }
    }

    for (period, by_index) in entries(get(profile, "bountyProgress")) {
        for (index, p) in entries(Some(by_index)) {
            let Ok(index) = index.parse::<i64>() else {
                continue;
            };
            let species: BTreeSet<i64> = entries(get(p, "species"))
                .iter()
                .filter_map(|(id, _)| id.parse().ok())
                .collect();
            let top: Vec<String> = entries(get(p, "top"))
                .iter()
                .map(|(_, w)| f_weight(w.as_num()))
                .collect();
            l.push(
                period_start(period, index),
                "JB",
                vec![
                    clean(period),
                    index.to_string(),
                    f_weight(num(p, "score")),
                    f_bit(yes(p, "specimen")),
                    species
                        .iter()
                        .map(i64::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                    top.join(","),
                ],
            );
        }
    }

    for (period, r) in entries(get(profile, "bountyRewardResults")) {
        let at = int(r, "at").unwrap_or(updated);
        let index = f_int(int(r, "period"));
        let mut standings: Vec<(i64, &Lua)> = entries(get(r, "standings"))
            .iter()
            .filter_map(|(rank, s)| rank.parse().ok().map(|rank| (rank, s)))
            .collect();
        standings.sort_by_key(|(rank, _)| *rank);
        l.push(
            at,
            "JX",
            vec![
                clean(period),
                index.clone(),
                f_int(int(r, "place")),
                f_weight(num(r, "score")),
                f_int(int(r, "xp")),
                standings.len().to_string(),
            ],
        );
        for (rank, s) in standings {
            l.push(
                at,
                "JY",
                vec![
                    clean(period),
                    index.clone(),
                    rank.to_string(),
                    text(s, "name")
                        .map(first_last_without_realm)
                        .unwrap_or_default(),
                    f_weight(num(s, "weight")),
                ],
            );
        }
    }

    for (item, f) in entries(get(profile, "finds")) {
        l.push(
            int(f, "at").unwrap_or(updated),
            "JF",
            vec![
                clean(item),
                f_str(text(f, "name")),
                f_int(int(f, "count")),
                f_str(text(f, "water")),
            ],
        );
    }

    for (_, f) in entries(get(db, "frenzyProgress")) {
        l.push(
            int(f, "at").unwrap_or(updated),
            "JQ",
            vec![
                f_int(int(f, "map")),
                f_coord(num(f, "x")),
                f_coord(num(f, "y")),
                f_int(int(f, "count")),
            ],
        );
    }

    if let Some(g) = get(profile, "higherLower") {
        let bests = get(g, "bests").unwrap_or(&empty);
        l.push(
            updated,
            "JG",
            vec![
                f_int(int(g, "runs")),
                f_int(int(g, "best")),
                f_int(int(g, "cleared")),
                f_str(text(g, "mode")),
                f_weight(num(bests, "weight")),
            ],
        );
    }
    l.out
}

#[derive(Default)]
struct Anglers {
    by_key: BTreeMap<String, String>,
    named: BTreeSet<String>,
}

impl Anglers {
    fn learn(&mut self, raw: &str) {
        let name = first_last_without_realm(raw);
        if !name.is_empty() {
            self.by_key
                .entry(angler_key_like_fishing_log(&name))
                .or_insert(name);
        }
    }

    fn name(&mut self, raw: &str) -> String {
        let name = first_last_without_realm(raw);
        if !name.is_empty() {
            self.named.insert(name.clone());
        }
        name
    }

    fn named_by_key(&mut self, key: &str) -> String {
        let name = self.by_key.get(key).cloned().unwrap_or_else(|| clean(key));
        self.named.insert(name.clone());
        name
    }
}

/// The `J` lines of a game account's file (`TheFishingLogDB`).
pub fn read_account(db: &Lua) -> Vec<Line> {
    let mut l = Lines::default();
    let empty = Lua::Table(Vec::new());
    let seen = int(db, "lastSeen").unwrap_or(0);
    let mut who = Anglers::default();

    for (_, spot) in entries(get(db, "spots")) {
        for table in ["fish", "season"] {
            for (_, r) in entries(get(spot, table)) {
                if let Some(h) = text(r, "h") {
                    who.learn(h);
                }
            }
        }
    }
    for (_, periods) in entries(get(db, "bountyBoard")) {
        for (_, board) in entries(Some(periods)) {
            for (_, e) in entries(Some(board)) {
                if let Some(n) = text(e, "name") {
                    who.learn(n);
                }
            }
        }
    }
    for (_, a) in entries(get(db, "anglersSeen")) {
        if let Some(n) = text(a, "name") {
            who.learn(n);
        }
    }
    for (_, d) in entries(get(db, "frenzyDiscoveries")) {
        for (_, a) in entries(get(d, "anglers")) {
            if let Some(n) = text(a, "name") {
                who.learn(n);
            }
        }
    }

    for (key, spot) in entries(get(db, "spots")) {
        let season = f_int(int(spot, "seasonID"));
        for (table, scope, season) in [("fish", "a", String::new()), ("season", "s", season)] {
            for (item, r) in entries(get(spot, table)) {
                l.push(
                    int(r, "t").unwrap_or(seen),
                    "JK",
                    vec![
                        clean(key),
                        scope.to_string(),
                        season.clone(),
                        clean(item),
                        f_weight(num(r, "w")),
                        text(r, "h").map(|h| who.name(h)).unwrap_or_default(),
                    ],
                );
            }
        }
    }

    let names = get(db, "spotNames");
    let wheres = get(db, "spotWhere");
    let keys: BTreeSet<&str> = entries(names)
        .iter()
        .chain(entries(wheres))
        .map(|(k, _)| k.as_str())
        .collect();
    for key in keys {
        let w = wheres.and_then(|t| get(t, key)).unwrap_or(&empty);
        l.push(
            seen,
            "JN",
            vec![
                clean(key),
                f_str(names.and_then(|t| text(t, key))),
                f_int(int(w, "map")),
                f_coord(num(w, "x")),
                f_coord(num(w, "y")),
                f_bit(yes(w, "mine")),
            ],
        );
    }

    for (item, name) in entries(get(db, "fishNames")) {
        l.push(seen, "JI", vec![clean(item), f_str(name.as_str())]);
    }

    for (board, periods) in entries(get(db, "bountyBoard")) {
        let (period, format) = board.split_once('@').unwrap_or((board, ""));
        for (index, entrants) in entries(Some(periods)) {
            let Ok(index) = index.parse::<i64>() else {
                continue;
            };
            for (_, e) in entries(Some(entrants)) {
                l.push(
                    period_start(period, index),
                    "JO",
                    vec![
                        clean(period),
                        clean(format),
                        index.to_string(),
                        text(e, "name").map(|n| who.name(n)).unwrap_or_default(),
                        f_weight(num(e, "score")),
                        f_bit(yes(e, "specimen")),
                    ],
                );
            }
        }
    }

    for (period, by_index) in entries(get(db, "bountySeen")) {
        for (index, t) in entries(Some(by_index)) {
            let Ok(index) = index.parse::<i64>() else {
                continue;
            };
            l.push(
                period_start(period, index),
                "JT",
                vec![
                    clean(period),
                    index.to_string(),
                    f_int(int(t, "id")),
                    f_str(text(t, "zone")),
                ],
            );
        }
    }

    for (_, a) in entries(get(db, "anglersSeen")) {
        l.push(
            int(a, "at").unwrap_or(seen),
            "JA",
            vec![
                text(a, "name").map(|n| who.name(n)).unwrap_or_default(),
                f_str(text(a, "guild")),
                f_int(int(a, "tflXP")),
                f_str(text(a, "version")),
                f_int(int(a, "tflXPAt")),
            ],
        );
    }

    for (_, d) in entries(get(db, "frenzyDiscoveries")) {
        for (_, a) in entries(get(d, "anglers")) {
            l.push(
                int(a, "at").unwrap_or(seen),
                "JD",
                vec![
                    f_str(text(d, "id")),
                    f_int(int(d, "map")),
                    f_coord(num(a, "x")),
                    f_coord(num(a, "y")),
                    text(a, "name").map(|n| who.name(n)).unwrap_or_default(),
                    f_int(int(a, "count")),
                    f_bit(yes(a, "mine")),
                ],
            );
        }
    }

    for (_, p) in entries(get(db, "poolAtlas")) {
        l.push(
            int(p, "at").unwrap_or(seen),
            "JU",
            vec![
                f_str(text(p, "name")),
                f_int(int(p, "map")),
                f_coord(num(p, "x")),
                f_coord(num(p, "y")),
                f_bit(yes(p, "mine")),
                entries(get(p, "by")).len().to_string(),
            ],
        );
    }

    for (zone, z) in entries(get(db, "zoneSurvey")) {
        for (item, f) in entries(get(z, "fish")) {
            l.push(
                seen,
                "JV",
                vec![
                    clean(zone),
                    f_int(int(z, "map")),
                    f_int(int(z, "total")),
                    clean(item),
                    f_str(text(f, "name")),
                    f_int(int(f, "c")),
                    f_int(int(f, "lo")),
                    f_int(int(f, "hi")),
                    f_bit(yes(f, "junk")),
                    f_bit(yes(f, "pool")),
                ],
            );
        }
        for (water, count) in entries(get(z, "waters")) {
            l.push(
                seen,
                "JC",
                vec![
                    clean(zone),
                    clean(water),
                    f_int(count.as_num().map(|n| n as i64)),
                ],
            );
        }
    }

    for (zone, items) in entries(get(db, "newZones")) {
        for (item, count) in entries(Some(items)) {
            l.push(
                seen,
                "JM",
                vec![
                    clean(zone),
                    clean(item),
                    f_int(count.as_num().map(|n| n as i64)),
                ],
            );
        }
    }

    for (map, kinds) in entries(get(db, "catchSkill")) {
        for (kind, bands) in entries(Some(kinds)) {
            for (band, b) in entries(Some(bands)) {
                l.push(
                    seen,
                    "JS",
                    vec![
                        clean(map),
                        clean(kind),
                        clean(band),
                        f_int(int(b, "l")),
                        f_int(int(b, "e")),
                    ],
                );
            }
        }
    }

    for (map, s) in entries(get(db, "skillUps")) {
        l.push(
            seen,
            "JL",
            vec![
                clean(map),
                String::new(),
                f_int(int(s, "lo")),
                f_int(int(s, "hi")),
                f_int(int(s, "n")),
            ],
        );
    }
    for (map, sources) in entries(get(db, "skillUpsHeard")) {
        for (source, s) in entries(Some(sources)) {
            let source = if source.starts_with('~') {
                clean(source)
            } else {
                who.named_by_key(source)
            };
            l.push(
                int(s, "at").unwrap_or(seen),
                "JL",
                vec![
                    clean(map),
                    source,
                    f_int(int(s, "lo")),
                    f_int(int(s, "hi")),
                    f_int(int(s, "n")),
                ],
            );
        }
    }

    for (key, until) in entries(get(db, "champions")) {
        let name = who.named_by_key(key);
        l.push(
            seen,
            "JCH",
            vec![name, f_int(until.as_num().map(|n| n as i64))],
        );
    }

    let factions = get(db, "factions");
    let named: Vec<String> = who.named.iter().cloned().collect();
    for name in named {
        if let Some(f) = factions.and_then(|t| text(t, &angler_key_like_fishing_log(&name))) {
            l.push(seen, "JE", vec![name, clean(f)]);
        }
    }
    l.out
}

/// What one character's files show: the `J` lines and the counts of its sessions.
#[derive(Default)]
pub struct Read {
    pub lines: Vec<Line>,
    pub counts: Vec<counts::Record>,
}

/// The counts of a character's fishing sessions: the `history` entries The Fishing Log wrote when a
/// session stopped (`solo`; match results and season summaries carry no such flag), each with the
/// counters the addon kept for it. A session without `elapsed` has no start.
pub fn session_counts(db: &Lua) -> Vec<counts::Record> {
    let mut out = Vec::new();
    for (_, h) in entries(get(db, "history")) {
        if yes(h, "solo") != Some(true) {
            continue;
        }
        let Some(to) = int(h, "when") else { continue };
        let mut fields = BTreeMap::new();
        for (name, v) in entries(get(h, "outcomes")) {
            let plain = name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
            if let (true, Some(n)) = (plain, v.as_num().filter(|n| n.is_finite())) {
                fields.insert(format!("outcomes.{name}"), n.round() as i64);
            }
        }
        for name in ["count", "duration", "elapsed"] {
            if let Some(n) = int(h, name) {
                fields.insert(name.to_string(), n);
            }
        }
        if let Some(spots) = get(h, "spots") {
            let junk = entries(Some(spots))
                .iter()
                .filter_map(|(_, w)| int(w, "junk"))
                .sum();
            fields.insert("spots.junk".to_string(), junk);
        }
        out.push(counts::Record {
            origin: counts::FISHING_LOG.to_string(),
            store: String::new(),
            scope: counts::CHARACTER.to_string(),
            window: counts::SESSION.to_string(),
            kind: counts::FISHING.to_string(),
            dims: String::new(),
            from: int(h, "elapsed").map(|e| to - e),
            to,
            fields,
        });
    }
    out
}

/// Each character's lines and session counts for one game account, in the order given. The account's
/// file goes to one character only, the one with the most lines of its own (the first by name on a
/// tie), so its record book, boards and anglers are not repeated for every alt. A character that
/// never fished gets no lines. An account file that does not read fails every character.
pub fn read_counted(
    account: Option<&Path>,
    chars: &[(String, PathBuf)],
) -> Vec<(String, Result<Read, String>)> {
    let acc_db = match account {
        Some(p) => fs::read_to_string(p)
            .map_err(|e| e.to_string())
            .and_then(|text| lua::parse_named(&text, "TheFishingLogDB")),
        None => Ok(None),
    };
    let mut out: Vec<(String, Result<Read, String>)> = chars
        .iter()
        .map(|(name, file)| {
            let read = match &acc_db {
                Ok(acc) => own_read(acc.as_ref(), file),
                Err(e) => Err(e.clone()),
            };
            (name.clone(), read)
        })
        .collect();
    let main = out
        .iter()
        .enumerate()
        .filter_map(|(i, (name, r))| r.as_ref().ok().map(|r| (r.lines.len(), name, i)))
        .filter(|(len, _, _)| *len > 0)
        .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(a.1)))
        .map(|(_, _, i)| i);
    if let (Some(i), Ok(Some(db))) = (main, &acc_db)
        && let Ok(read) = &mut out[i].1
    {
        read.lines.extend(read_account(db));
    }
    for (_, r) in &mut out {
        if let Ok(read) = r {
            read.lines.sort();
            read.lines.dedup();
        }
    }
    out
}

fn own_read(account: Option<&Lua>, character: &Path) -> Result<Read, String> {
    let text = fs::read_to_string(character).map_err(|e| e.to_string())?;
    let Some(db) = lua::parse_named(&text, "TheFishingLogChar")? else {
        return Ok(Read::default());
    };
    let ids = fish_ids(account, Some(&db));
    Ok(Read {
        lines: read_char(&db, &ids)
            .into_iter()
            .filter(|l| !is_placeholder(l))
            .collect(),
        counts: session_counts(&db),
    })
}

fn is_placeholder(l: &Line) -> bool {
    let blank = |i: usize| matches!(l.fields.get(i).map(String::as_str), None | Some("" | "0"));
    match l.code {
        "JP" => blank(0),
        "JB" => blank(2) && blank(4) && blank(5),
        _ => false,
    }
}

/// The payload text: `scarletfishing<TAB>1<TAB>Name-Surname`, then `<epoch ms><TAB><code><TAB><fields>`.
pub fn scl_text(name: &str, lines: &[Line]) -> String {
    let mut out = format!("{MAGIC}\t1\t{name}\n");
    for l in lines {
        out.push_str(&l.ms.to_string());
        out.push('\t');
        out.push_str(l.code);
        for f in &l.fields {
            out.push('\t');
            out.push_str(f);
        }
        out.push('\n');
    }
    out
}

/// Write one character's `fishing/payloads/fishing.scl`, or remove it when there are no lines.
pub fn write(
    char_dir: &Path,
    name: &str,
    lines: &[Line],
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = char_dir.join(DIR).join("payloads");
    let scl = dir.join("fishing.scl");
    if lines.is_empty() {
        let _ = fs::remove_file(&scl);
        return Ok(());
    }
    fs::create_dir_all(&dir)?;
    let payload = zstd::stream::encode_all(scl_text(name, lines).as_bytes(), 19)?;
    let tmp = scl.with_extension("scl.tmp");
    fs::write(&tmp, payload)?;
    fs::rename(&tmp, &scl)?;
    Ok(())
}

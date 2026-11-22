//! The in-memory store: every .scl payload under the data folder, merged,
//! sorted by time, with name and zone maps, sessions, timeline lines and map
//! polylines. Loaded once, swapped atomically after an export.

use crate::layers::{self, Layers};
use scarlet::addons;
use scarlet::archive::{self, Event};
use scarlet::events::{self, Ev};
use scarlet::lifetime::{self, Lifetime};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

/// One event with its character index.
pub struct Row {
    pub ms: i64,
    pub ch: u32,
    pub code: String,
    pub fields: Vec<String>,
}

/// A timeline line: everything except P, SN and IN, rendered like
/// scarlet.txt (`word` + `text`, timestamp added by the UI).
pub struct TLine {
    pub ms: i64,
    pub ch: u32,
    pub code: String,
    pub word: String,
    pub text: String,
}

#[derive(Clone)]
pub struct Session {
    pub ch: u32,
    pub start: i64,
    pub end: i64,
    pub zones: Vec<String>,
    pub kills: u64,
    pub xp: i64,
    pub money: i64,
    pub casts: u64,
}

pub struct MapLine {
    pub map: u32,
    pub session: u32,
    pub pts: Vec<(f32, f32)>,
    /// A teleport on one map: two points drawn as a dashed jump, never merged into a path.
    pub dash: bool,
}

pub struct Store {
    pub data_dir: PathBuf,
    pub chars: Vec<String>,
    pub events: Vec<Row>,
    pub lifetimes: Vec<Lifetime>,
    pub lines: Vec<TLine>,
    pub sessions: Vec<Session>,
    /// uiMapID -> name, from Z lines.
    pub zones: BTreeMap<u32, String>,
    /// Name map: `n`/`s`/`i` + id -> name.
    pub names: HashMap<String, String>,
    /// Polylines grouped by map, in time order.
    pub polylines: Vec<MapLine>,
    /// The Survey look's layers per (character, map).
    pub layers: Layers,
    /// Code -> count, for the filter chips.
    pub code_counts: BTreeMap<String, u64>,
    pub first_ms: i64,
    pub last_ms: i64,
}

fn ev_to_parts(ms: i64, e: &Event, names: &HashMap<String, String>) -> (String, String) {
    let ev = Ev {
        cs: ms / 10,
        code: e.code.clone(),
        fields: e.fields.clone(),
    };
    events::event_parts(&ev, names)
}

impl Store {
    /// Read every payload for every character under `dir`.
    pub fn load(dir: &Path) -> Store {
        let mut chars: Vec<String> = Vec::new();
        let mut rows: Vec<Row> = Vec::new();
        let mut names: HashMap<String, String> = HashMap::new();
        let mut zones: BTreeMap<u32, String> = BTreeMap::new();
        let mut code_counts: BTreeMap<String, u64> = BTreeMap::new();
        let mut sessions: Vec<Session> = Vec::new();
        let mut polylines: Vec<MapLine> = Vec::new();
        let mut layer_data = Layers::new();

        let Ok(entries) = std::fs::read_dir(dir) else {
            return Store::empty(dir);
        };
        let mut char_dirs: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir() && p.join("payloads").is_dir())
            .collect();
        char_dirs.sort();

        for cd in char_dirs {
            let name = cd
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let Ok(payloads) = archive::char_payloads(&cd) else {
                continue;
            };
            if payloads.is_empty() {
                continue;
            }
            let ch = chars.len() as u32;
            chars.push(name);

            // Pass 1: names and zones, so pass 2 renders with them.
            let mut parsed: Vec<Event> = Vec::new();
            let mut fishing: HashSet<String> = HashSet::new();
            for p in &payloads {
                let Ok(pl) = archive::read_payload(p) else {
                    continue;
                };
                for e in pl.events {
                    if e.code == "SN"
                        && let [id, name, ..] = e.fields.as_slice()
                        && name == "Fishing"
                    {
                        fishing.insert(id.clone());
                    }
                    if let Some((kind, id, n)) = archive::name_of(&e) {
                        names.insert(events::name_key(kind, &id), n);
                    } else if e.code == "Z"
                        && let (Some(map), Some(zone)) =
                            (e.fields.first(), e.fields.get(4).filter(|z| !z.is_empty()))
                        && let Ok(m) = map.parse::<u32>()
                    {
                        zones.entry(m).or_insert_with(|| zone.clone());
                    }
                    parsed.push(e);
                }
            }

            // Pass 2: rows, sessions and polylines.
            let mut cur_session: u32 = u32::MAX;
            let mut open: Option<usize> = None;
            let mut feed = layers::Feed::new(ch, fishing);
            for mut e in parsed {
                feed.line(&mut layer_data, e.ms, &e.code, &e.fields);
                if !matches!(e.code.as_str(), "P" | "SN" | "IN") {
                    *code_counts.entry(e.code.clone()).or_default() += 1;
                }
                match e.code.as_str() {
                    "P" => {
                        if let [map, x, y, ..] = e.fields.as_slice()
                            && let (Ok(m), Ok(x), Ok(y)) =
                                (map.parse::<u32>(), x.parse::<i64>(), y.parse::<i64>())
                        {
                            polylines.push(MapLine {
                                map: m,
                                session: cur_session,
                                pts: vec![(x as f32 / 100.0, y as f32 / 100.0)],
                                dash: false,
                            });
                        }
                        continue;
                    }
                    // TE fromMap fromX fromY toMap toX toY: a teleport inside one map is a dashed jump.
                    "TE" => {
                        if let [_, _, _, fm, fx, fy, tm, tx, ty, ..] = e.fields.as_slice()
                            && fm == tm
                            && let (Ok(m), Ok(fx), Ok(fy), Ok(tx), Ok(ty)) = (
                                fm.parse::<u32>(),
                                fx.parse::<f32>(),
                                fy.parse::<f32>(),
                                tx.parse::<f32>(),
                                ty.parse::<f32>(),
                            )
                        {
                            polylines.push(MapLine {
                                map: m,
                                session: cur_session,
                                pts: vec![(fx / 100.0, fy / 100.0), (tx / 100.0, ty / 100.0)],
                                dash: true,
                            });
                        }
                    }
                    "S" => {
                        cur_session = sessions.len() as u32;
                        open = Some(sessions.len());
                        sessions.push(Session {
                            ch,
                            start: e.ms,
                            end: e.ms,
                            zones: vec![],
                            kills: 0,
                            xp: 0,
                            money: 0,
                            casts: 0,
                        });
                    }
                    "ES" => {
                        if let Some(i) = open.take() {
                            sessions[i].end = e.ms;
                        }
                        cur_session = u32::MAX;
                    }
                    _ => {}
                }
                if let Some(i) = open {
                    let s = &mut sessions[i];
                    // A session cut off without ES ends at its last event.
                    s.end = s.end.max(e.ms);
                    match e.code.as_str() {
                        "K" => s.kills += 1,
                        "C" => s.casts += 1,
                        "X" => s.xp += e.fields.first().and_then(|f| f.parse().ok()).unwrap_or(0),
                        "M" => {
                            s.money += e.fields.first().and_then(|f| f.parse().ok()).unwrap_or(0)
                        }
                        "Z" => {
                            if let Some(z) = e.fields.get(4).filter(|z| !z.is_empty()) {
                                let last = s.zones.last();
                                if last != Some(&z.to_string()) {
                                    s.zones.push(z.clone());
                                }
                            }
                        }
                        _ => {}
                    }
                }
                rows.push(Row {
                    ms: e.ms,
                    ch,
                    code: std::mem::take(&mut e.code),
                    fields: std::mem::take(&mut e.fields),
                });
            }
            for ev in addons::read_payloads(&cd) {
                feed.addon(&mut layer_data, &ev.key, &ev.code, &ev.fields);
            }
        }

        rows.sort_by_key(|r| r.ms);
        polylines.sort_by_key(|p| p.session);

        // Merge consecutive P points into one polyline per (map, session).
        let mut merged: Vec<MapLine> = Vec::new();
        for pl in polylines {
            match merged.last_mut() {
                Some(last)
                    if !last.dash
                        && !pl.dash
                        && last.map == pl.map
                        && last.session == pl.session =>
                {
                    last.pts.extend(pl.pts);
                }
                _ => merged.push(pl),
            }
        }

        let first_ms = rows.first().map(|r| r.ms).unwrap_or(0);
        let last_ms = rows.last().map(|r| r.ms).unwrap_or(0);

        // Timeline lines: rendered after all names are known.
        let mut lines: Vec<TLine> = Vec::new();
        for r in &rows {
            if matches!(r.code.as_str(), "P" | "SN" | "IN") {
                continue;
            }
            let e = Event {
                ms: r.ms,
                code: r.code.clone(),
                fields: r.fields.clone(),
            };
            let (word, text) = ev_to_parts(r.ms, &e, &names);
            lines.push(TLine {
                ms: r.ms,
                ch: r.ch,
                code: r.code.clone(),
                word,
                text,
            });
        }

        let lifetimes = (0..chars.len() as u32)
            .map(|ch| {
                lifetime::build(
                    rows.iter()
                        .filter(|r| r.ch == ch)
                        .map(|r| (r.ms, r.code.as_str(), r.fields.as_slice())),
                )
            })
            .collect();

        Store {
            data_dir: dir.to_path_buf(),
            chars,
            events: rows,
            lifetimes,
            lines,
            sessions,
            zones,
            names,
            polylines: merged,
            layers: layer_data,
            code_counts,
            first_ms,
            last_ms,
        }
    }

    fn empty(dir: &Path) -> Store {
        Store {
            data_dir: dir.to_path_buf(),
            chars: vec![],
            events: vec![],
            lifetimes: vec![],
            lines: vec![],
            sessions: vec![],
            zones: BTreeMap::new(),
            names: HashMap::new(),
            polylines: vec![],
            layers: Layers::new(),
            code_counts: BTreeMap::new(),
            first_ms: 0,
            last_ms: 0,
        }
    }

    pub fn char_index(&self, name: &str) -> Option<u32> {
        self.chars.iter().position(|c| c == name).map(|i| i as u32)
    }
}

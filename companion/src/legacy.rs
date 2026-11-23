//! A character's Legacy talents from its LN, LE and LB lines: every talent of the three trees
//! with its rank over time and what it says it does at each rank, and, for the talents whose
//! effect other lines show, that effect measured at each rank. Feeds the app's Legacy view.

use crate::archive::Event;
use crate::tables::csv;
use crate::talents::Line;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};

pub const CODES: [&str; 16] = [
    "S", "LN", "LE", "LB", "SN", "LO", "O", "WE", "A", "RX", "RO", "SK", "SL", "KN", "C", "ES",
];

const LUREMASTER: &str = "1225455";
const WORKING_OVERTIME: &str = "1225451";
const WELL_RESTED: &str = "1225478";
const GOURMAND: &str = "1225499";

const BOBBER: &str = "35591";
const CATCH_ITEMS_MS: i64 = 1_500;
const LURE_SKILL_MS: i64 = 5_000;
const SKILL_UP_BEFORE_MS: i64 = 1_500;
const SKILL_UP_AFTER_MS: i64 = 3_000;
const LOGIN_RESTED_MS: i64 = 60_000;
const MIN_REST_HOURS: f64 = 0.5;
const RESTED_CAP_SHARE: f64 = 1.45;

const GATHER_CASTS: [(&str, &str); 4] = [
    ("Mining", "Mining"),
    ("Herbalism", "Herbalism"),
    ("Herb Gathering", "Herbalism"),
    ("Skinning", "Skinning"),
];
const GATHER_SPELLS: [(&str, &str); 3] = [
    ("2575", "Mining"),
    ("2576", "Mining"),
    ("11993", "Herbalism"),
];
fn field<'a>(l: &Line<'a>, i: usize) -> &'a str {
    l.fields.get(i).map(String::as_str).unwrap_or("")
}

fn number(s: &str) -> Option<u32> {
    s.trim().parse().ok()
}

fn decimal(s: &str) -> Option<f64> {
    s.trim().parse().ok()
}

fn tree_name(id: &str) -> &'static str {
    match id {
        "1187" => "Professions",
        "1188" => "Adventure",
        "1189" => "Progression",
        _ => "Other",
    }
}

fn stated(text: &str) -> Option<f64> {
    let end = text.find('%')?;
    let start = text[..end]
        .char_indices()
        .rfind(|(_, c)| !(c.is_ascii_digit() || *c == '.'))
        .map_or(0, |(i, c)| i + c.len_utf8());
    text[start..end].parse().ok()
}

#[derive(Serialize)]
pub struct RankText {
    pub rank: u32,
    pub text: String,
    pub stated: Option<f64>,
}

#[derive(Serialize, Clone, Copy)]
pub struct RankChange {
    pub ms: i64,
    /// Empty on the first rank Scarlet read for the character.
    pub from: Option<u32>,
    pub to: u32,
}

/// `chance`: each trial hits or not; value is the share that hit, in percent, and effect the
/// difference from rank 0 in points. `increase`: value is the mean, effect the percent above rank 0.
#[derive(Serialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Chance,
    Increase,
}

#[derive(Serialize)]
pub struct Row {
    pub rank: u32,
    pub trials: u32,
    pub value: Option<f64>,
    pub effect: Option<f64>,
    pub stated: Option<f64>,
    pub first_ms: i64,
    pub last_ms: i64,
}

#[derive(Serialize)]
pub struct Measure {
    pub what: &'static str,
    pub trials: &'static str,
    pub unit: &'static str,
    pub kind: Kind,
    pub rows: Vec<Row>,
}

#[derive(Serialize)]
pub struct Talent {
    pub tree: &'static str,
    pub tree_id: String,
    pub node_id: String,
    pub spell_id: String,
    pub name: String,
    pub rank: u32,
    pub max_ranks: u32,
    pub texts: Vec<RankText>,
    pub changes: Vec<RankChange>,
    pub measure: Option<Measure>,
}

#[derive(Serialize, Default)]
pub struct Legacy {
    /// The first Legacy line: ranks before it are unknown and nothing before it is measured.
    pub known_since: Option<i64>,
    pub talents: Vec<Talent>,
}

#[derive(Default)]
struct Node {
    tree_id: String,
    spell_id: String,
    name: String,
    max_ranks: u32,
    ranks: Vec<(i64, u32)>,
    changes: Vec<RankChange>,
    entry: String,
    entries: BTreeMap<String, (String, BTreeMap<u32, String>)>,
}

impl Node {
    fn rank(&self) -> u32 {
        self.ranks.last().map_or(0, |r| r.1)
    }

    fn set(&mut self, ms: i64, rank: u32, first: bool) {
        let from = if first { None } else { Some(self.rank()) };
        if from == Some(rank) {
            return;
        }
        self.ranks.push((ms, rank));
        self.changes.push(RankChange { ms, from, to: rank });
    }

    fn texts(&self) -> Option<&(String, BTreeMap<u32, String>)> {
        self.entries
            .get(&self.entry)
            .or_else(|| self.entries.values().next())
    }

    fn text(&self, rank: u32) -> Option<&String> {
        self.texts().and_then(|t| t.1.get(&rank))
    }

    fn rank_at(&self, ms: i64, known_since: Option<i64>) -> Option<u32> {
        if known_since.is_none_or(|k| ms < k) {
            return None;
        }
        let i = self.ranks.partition_point(|r| r.0 <= ms);
        Some(if i == 0 { 0 } else { self.ranks[i - 1].1 })
    }
}

struct Ranks {
    nodes: BTreeMap<String, Node>,
    known_since: Option<i64>,
}

fn ranks<'a>(lines: &[Line<'a>]) -> Ranks {
    let mut nodes: BTreeMap<String, Node> = BTreeMap::new();
    let mut known_since = None;
    let mut session_base: HashSet<&str> = HashSet::new();
    let mut trees_based: HashSet<&str> = HashSet::new();
    for l in lines {
        match l.code {
            "S" => {
                session_base.clear();
                trees_based.clear();
            }
            "LN" => {
                let id = field(l, 1);
                let Some(rank) = number(field(l, 3)) else {
                    continue;
                };
                let first = known_since.is_none_or(|k| k == l.ms);
                known_since.get_or_insert(l.ms);
                let n = nodes.entry(id.to_string()).or_default();
                n.tree_id = field(l, 0).to_string();
                n.max_ranks = number(field(l, 6)).unwrap_or(n.max_ranks);
                if !field(l, 7).is_empty() {
                    n.spell_id = field(l, 7).to_string();
                }
                if !field(l, 2).is_empty() {
                    n.entry = field(l, 2).to_string();
                }
                if !field(l, 9).is_empty() {
                    n.name = field(l, 9).to_string();
                }
                if field(l, 8) == "s" {
                    session_base.insert(id);
                    trees_based.insert(field(l, 0));
                }
                n.set(l.ms, rank, first);
            }
            "LB" if field(l, 8) == "s" => {
                if known_since.is_none() {
                    known_since = Some(l.ms);
                    continue;
                }
                let spent = |tree: &str| match tree {
                    "1187" => field(l, 5),
                    "1188" => field(l, 6),
                    "1189" => field(l, 7),
                    _ => "",
                };
                for (id, n) in nodes.iter_mut() {
                    let tree = n.tree_id.as_str();
                    let read = trees_based.contains(tree) || spent(tree) == "0";
                    if read && n.rank() > 0 && !session_base.contains(id.as_str()) {
                        n.set(l.ms, 0, false);
                    }
                }
            }
            "LE" => {
                let Some(rank) = number(field(l, 4)) else {
                    continue;
                };
                let n = nodes.entry(field(l, 1).to_string()).or_default();
                n.tree_id = field(l, 0).to_string();
                n.max_ranks = number(field(l, 5)).unwrap_or(n.max_ranks);
                let e = n.entries.entry(field(l, 2).to_string()).or_default();
                if !field(l, 3).is_empty() {
                    e.0 = field(l, 3).to_string();
                }
                e.1.insert(rank, field(l, 6).to_string());
            }
            _ => {}
        }
    }
    Ranks { nodes, known_since }
}

struct Samples {
    what: &'static str,
    trials: &'static str,
    unit: &'static str,
    kind: Kind,
    points: Vec<(i64, f64)>,
}

fn spell_names<'a>(lines: &[Line<'a>]) -> HashMap<&'a str, &'a str> {
    lines
        .iter()
        .filter(|l| l.code == "SN")
        .map(|l| (field(l, 0), field(l, 1)))
        .collect()
}

fn lure_enchants<'a>(lines: &[Line<'a>]) -> HashSet<&'a str> {
    let fishing: HashSet<&str> = lines
        .iter()
        .filter(|l| l.code == "KN" && field(l, 1) == "Fishing")
        .map(|l| field(l, 0))
        .collect();
    let mut lures = HashSet::new();
    let mut applied: Option<(i64, &str)> = None;
    let mut modifier: HashMap<&str, u32> = HashMap::new();
    for l in lines {
        match l.code {
            "WE" if field(l, 0) == "0" && number(field(l, 1)).is_some_and(|e| e > 0) => {
                applied = Some((l.ms, field(l, 1)));
            }
            "SL" if fishing.contains(field(l, 0)) => {
                let now = number(field(l, 3)).unwrap_or(0);
                let was = modifier.insert(field(l, 0), now).unwrap_or(0);
                if now > was
                    && let Some((ms, enchant)) = applied
                    && l.ms - ms <= LURE_SKILL_MS
                {
                    lures.insert(enchant);
                }
            }
            _ => {}
        }
    }
    lures
}

fn luremaster(lines: &[Line]) -> Samples {
    let lures = lure_enchants(lines);
    let mut points = Vec::new();
    let mut lure_until: Option<i64> = None;
    let mut open: Option<(i64, u32)> = None;
    let close = |open: &mut Option<(i64, u32)>, points: &mut Vec<(i64, f64)>| {
        if let Some((ms, items)) = open.take() {
            points.push((ms, if items > 1 { 1.0 } else { 0.0 }));
        }
    };
    for l in lines {
        match l.code {
            "WE" if field(l, 0) == "0" => {
                lure_until = match number(field(l, 1)) {
                    Some(e) if e > 0 && lures.contains(field(l, 1)) => {
                        Some(decimal(field(l, 3)).map_or(i64::MAX, |s| l.ms + (s * 1000.0) as i64))
                    }
                    _ => None,
                };
            }
            "LO" if field(l, 0) == "g" && field(l, 1) == BOBBER => {
                close(&mut open, &mut points);
                if lure_until.is_some_and(|u| l.ms <= u) {
                    open = Some((l.ms, 0));
                }
            }
            "O" => {
                if let Some((ms, items)) = open.as_mut()
                    && l.ms - *ms <= CATCH_ITEMS_MS
                {
                    *items += number(field(l, 1)).unwrap_or(1);
                }
            }
            _ => {}
        }
    }
    close(&mut open, &mut points);
    Samples {
        what: "Lure catches with more than one item",
        trials: "Catches",
        unit: "%",
        kind: Kind::Chance,
        points,
    }
}

fn gourmand(lines: &[Line], names: &HashMap<&str, &str>) -> Samples {
    let points = lines
        .iter()
        .filter(|l| l.code == "A" && field(l, 0) == "1" && field(l, 5) != "s")
        .filter(|l| names.get(field(l, 1)) == Some(&"Well Fed"))
        .filter_map(|l| {
            decimal(field(l, 3))
                .filter(|d| *d > 0.0)
                .map(|d| (l.ms, d / 60.0))
        })
        .collect();
    Samples {
        what: "Well Fed duration",
        trials: "Buffs",
        unit: "min",
        kind: Kind::Increase,
        points,
    }
}

fn well_rested(lines: &[Line]) -> Samples {
    let mut points = Vec::new();
    let mut last_out: Option<(i64, f64, f64)> = None;
    let mut waiting: Option<(i64, f64, f64, i64)> = None;
    for l in lines {
        match l.code {
            "RO" => {
                last_out = match (decimal(field(l, 0)), decimal(field(l, 1)), field(l, 2)) {
                    (Some(rest), Some(max), "1") if max > 0.0 => Some((l.ms, rest, max)),
                    _ => None,
                };
            }
            "S" => {
                waiting = last_out.take().map(|(ms, rest, max)| (ms, rest, max, l.ms));
            }
            "RX" => {
                let Some((out_ms, rest, max, in_ms)) = waiting.take() else {
                    continue;
                };
                if l.ms - in_ms > LOGIN_RESTED_MS {
                    continue;
                }
                let now = if field(l, 0) == "1" {
                    decimal(field(l, 1)).unwrap_or(0.0)
                } else {
                    0.0
                };
                let hours = (in_ms - out_ms) as f64 / 3_600_000.0;
                if hours >= MIN_REST_HOURS && now >= rest && now < RESTED_CAP_SHARE * max {
                    points.push((out_ms, (now - rest) / max * 100.0 / hours * 8.0));
                }
            }
            _ => {}
        }
    }
    Samples {
        what: "Rested XP gained logged out in a rest area, % of a level per 8 hours",
        trials: "Logouts",
        unit: "%",
        kind: Kind::Increase,
        points,
    }
}

fn working_overtime(lines: &[Line], names: &HashMap<&str, &str>) -> Samples {
    let mut skill_names: HashMap<&str, &str> = HashMap::new();
    let mut ups: Vec<(i64, &str)> = Vec::new();
    for l in lines {
        match l.code {
            "KN" => {
                skill_names.insert(field(l, 0), field(l, 1));
            }
            "SK" => ups.push((l.ms, field(l, 0))),
            _ => {}
        }
    }
    let mut used: HashSet<usize> = HashSet::new();
    let mut capped: HashMap<&str, bool> = HashMap::new();
    let mut points = Vec::new();
    for l in lines {
        let prof = match l.code {
            "SL" => {
                if let (Some(name), Some(rank), Some(max)) = (
                    skill_names.get(field(l, 0)),
                    number(field(l, 1)),
                    number(field(l, 2)),
                ) {
                    capped.insert(*name, max > 0 && rank >= max);
                }
                continue;
            }
            "C" => GATHER_SPELLS
                .iter()
                .find(|(id, _)| *id == field(l, 0))
                .map(|(_, p)| *p)
                .or_else(|| {
                    names.get(field(l, 0)).and_then(|n| {
                        GATHER_CASTS
                            .iter()
                            .find(|(cast, _)| cast == n)
                            .map(|(_, p)| *p)
                    })
                }),
            "LO" if field(l, 0) == "g" && field(l, 1) == BOBBER => Some("Fishing"),
            _ => None,
        };
        let Some(prof) = prof else { continue };
        if capped.get(prof) == Some(&true) {
            continue;
        }
        let lo = ups.partition_point(|u| u.0 < l.ms - SKILL_UP_BEFORE_MS);
        let hit = (lo..ups.len())
            .take_while(|&i| ups[i].0 <= l.ms + SKILL_UP_AFTER_MS)
            .find(|&i| ups[i].1 == prof && !used.contains(&i));
        if let Some(i) = hit {
            used.insert(i);
        }
        points.push((l.ms, if hit.is_some() { 100.0 } else { 0.0 }));
    }
    Samples {
        what: "Gathering casts and fishing catches below the skill cap that gave a skill-up",
        trials: "Actions",
        unit: "%",
        kind: Kind::Increase,
        points,
    }
}

fn measure(node: &Node, s: Samples, known_since: Option<i64>) -> Measure {
    let mut by_rank: BTreeMap<u32, (u32, f64, i64, i64)> = BTreeMap::new();
    for (ms, v) in &s.points {
        let Some(rank) = node.rank_at(*ms, known_since) else {
            continue;
        };
        let e = by_rank.entry(rank).or_insert((0, 0.0, *ms, *ms));
        e.0 += 1;
        e.1 += v;
        e.2 = e.2.min(*ms);
        e.3 = e.3.max(*ms);
    }
    let scale = if s.kind == Kind::Chance { 100.0 } else { 1.0 };
    let value = |r: &(u32, f64, i64, i64)| (r.0 > 0).then(|| r.1 / r.0 as f64 * scale);
    let base = by_rank.get(&0).and_then(value);
    let rows = (0..=node
        .max_ranks
        .max(by_rank.keys().max().copied().unwrap_or(0)))
        .map(|rank| {
            let r = by_rank.get(&rank);
            let v = r.and_then(value);
            let effect = match (v, base) {
                (Some(v), Some(b)) if rank > 0 => match s.kind {
                    Kind::Chance => Some(v - b),
                    Kind::Increase if b > 0.0 => Some((v / b - 1.0) * 100.0),
                    Kind::Increase => None,
                },
                _ => None,
            };
            Row {
                rank,
                trials: r.map_or(0, |r| r.0),
                value: v,
                effect,
                stated: if rank == 0 {
                    None
                } else {
                    node.text(rank).and_then(|t| stated(t))
                },
                first_ms: r.map_or(0, |r| r.2),
                last_ms: r.map_or(0, |r| r.3),
            }
        })
        .collect();
    Measure {
        what: s.what,
        trials: s.trials,
        unit: s.unit,
        kind: s.kind,
        rows,
    }
}

pub fn build<'a>(lines: impl IntoIterator<Item = Line<'a>>) -> Legacy {
    let lines: Vec<Line<'a>> = lines.into_iter().collect();
    let Ranks { nodes, known_since } = ranks(&lines);
    if nodes.is_empty() {
        return Legacy::default();
    }
    let names = spell_names(&lines);
    let mut talents: Vec<Talent> = nodes
        .iter()
        .map(|(id, n)| {
            let spell_id = if n.spell_id.is_empty() {
                n.texts().map(|t| t.0.clone()).unwrap_or_default()
            } else {
                n.spell_id.clone()
            };
            let samples = match spell_id.as_str() {
                LUREMASTER => Some(luremaster(&lines)),
                GOURMAND => Some(gourmand(&lines, &names)),
                WELL_RESTED => Some(well_rested(&lines)),
                WORKING_OVERTIME => Some(working_overtime(&lines, &names)),
                _ => None,
            };
            let name = names
                .get(spell_id.as_str())
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty())
                .or_else(|| (!n.name.is_empty()).then(|| n.name.clone()))
                .unwrap_or_else(|| format!("Node {id}"));
            Talent {
                tree: tree_name(&n.tree_id),
                tree_id: n.tree_id.clone(),
                node_id: id.clone(),
                spell_id,
                name,
                rank: n.rank(),
                max_ranks: n.max_ranks,
                texts: n
                    .texts()
                    .into_iter()
                    .flat_map(|t| t.1.iter())
                    .map(|(rank, text)| RankText {
                        rank: *rank,
                        text: text.clone(),
                        stated: stated(text),
                    })
                    .collect(),
                changes: n.changes.clone(),
                measure: samples.map(|s| measure(n, s, known_since)),
            }
        })
        .collect();
    talents.sort_by(|a, b| {
        a.tree_id
            .cmp(&b.tree_id)
            .then(b.rank.cmp(&a.rank))
            .then(b.measure.is_some().cmp(&a.measure.is_some()))
            .then(a.name.cmp(&b.name))
    });
    Legacy {
        known_since,
        talents,
    }
}

pub fn from_events(events: &[Event]) -> Legacy {
    build(
        events
            .iter()
            .filter(|e| CODES.contains(&e.code.as_str()))
            .map(|e| Line {
                ms: e.ms,
                code: &e.code,
                fields: &e.fields,
            }),
    )
}

impl Legacy {
    /// One row per rank of each talent: its text, and the measure at that rank when it has one.
    pub fn csv(&self) -> String {
        let mut out = String::from(
            "tree,node_id,spell_id,talent,rank_now,max_ranks,rank,text,stated,measure,trials,value,unit,effect,first_ms,last_ms
",
        );
        let num = |v: Option<f64>| v.map(|n| format!("{n:.2}")).unwrap_or_default();
        for t in &self.talents {
            for rank in 0..=t.max_ranks {
                let text = t.texts.iter().find(|x| x.rank == rank);
                let row = t
                    .measure
                    .as_ref()
                    .and_then(|m| m.rows.iter().find(|r| r.rank == rank).map(|r| (m, r)));
                if text.is_none() && row.is_none_or(|(_, r)| r.trials == 0) {
                    continue;
                }
                out.push_str(&csv(&[
                    t.tree.to_string(),
                    t.node_id.clone(),
                    t.spell_id.clone(),
                    t.name.clone(),
                    t.rank.to_string(),
                    t.max_ranks.to_string(),
                    rank.to_string(),
                    text.map(|x| x.text.clone()).unwrap_or_default(),
                    num(text.and_then(|x| x.stated)),
                    row.map(|(m, _)| m.what.to_string()).unwrap_or_default(),
                    row.map(|(_, r)| r.trials.to_string()).unwrap_or_default(),
                    num(row.and_then(|(_, r)| r.value)),
                    row.map(|(m, _)| m.unit.to_string()).unwrap_or_default(),
                    num(row.and_then(|(_, r)| r.effect)),
                    row.filter(|(_, r)| r.trials > 0)
                        .map(|(_, r)| r.first_ms.to_string())
                        .unwrap_or_default(),
                    row.filter(|(_, r)| r.trials > 0)
                        .map(|(_, r)| r.last_ms.to_string())
                        .unwrap_or_default(),
                ]));
            }
        }
        out
    }
}

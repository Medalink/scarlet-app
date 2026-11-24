//! What a character is working toward and how well it does at it, from its own history only: the rates and records of
//! its WoW runs, and how far each skill, reputation and Legacy tree is from its ceiling.

use crate::events::name_key;
use crate::overview::{Ev, balances};
use crate::recap::{Activity, Facts, Spread, activity};
use serde::Serialize;
use std::collections::HashMap;

const MIN_PLAYED: i64 = 900;
const MIN_PEERS: usize = 3;
const BAND: u32 = 2;
const EXALTED: i64 = 42_000;
const BELOW_NEUTRAL: [(i64, i64); 3] = [(-42_000, -6_000), (-6_000, -3_000), (-3_000, 0)];
const PROFESSIONS: [&str; 17] = [
    "Alchemy",
    "Blacksmithing",
    "Enchanting",
    "Engineering",
    "Herbalism",
    "Jewelcrafting",
    "Leatherworking",
    "Mining",
    "Skinning",
    "Tailoring",
    "Inscription",
    "Cooking",
    "Fishing",
    "First Aid",
    "Archaeology",
    "Riding",
    "Lockpicking",
];

struct Metric {
    key: &'static str,
    banded: bool,
    value: fn(&Activity, u32) -> Option<f64>,
}

impl Metric {
    fn of(&self, a: &Activity, cap: u32) -> Option<f64> {
        (a.played > 0).then(|| (self.value)(a, cap)).flatten()
    }
}

fn per_hour(n: i64, a: &Activity) -> f64 {
    n as f64 * 3600.0 / a.played as f64
}

const METRICS: [Metric; 4] = [
    Metric {
        key: "xp_h",
        banded: true,
        value: |a, cap| (a.xp > 0 && (cap == 0 || a.level[0] < cap)).then(|| per_hour(a.xp, a)),
    },
    Metric {
        key: "gold_h",
        banded: true,
        value: |a, _| (a.net != 0 || a.coin != 0).then(|| per_hour(a.net, a)),
    },
    Metric {
        key: "kills_h",
        banded: true,
        value: |a, _| (a.kills > 0).then(|| per_hour(a.kills, a)),
    },
    Metric {
        key: "gathers_h",
        banded: false,
        value: |a, _| (a.gathers > 0).then(|| per_hour(a.gathers, a)),
    },
];

type Total = fn(&Activity) -> i64;

const TOTALS: [(&str, Total); 6] = [
    ("xp", |a| a.xp),
    ("coin", |a| a.coin),
    ("kills", |a| a.kills),
    ("gathers", |a| a.gathers),
    ("item", |a| a.top_item.map_or(0, |t| t.1)),
    ("played", |a| a.played),
];

/// One WoW run of the character with what it did.
pub struct Row {
    pub from: i64,
    pub to: i64,
    a: Activity,
    top_name: String,
}

impl Row {
    pub fn of(f: &Facts, spans: &[(i64, i64)]) -> Option<Row> {
        let spans = crate::recap::union_spans(spans);
        let from = spans.first()?.0;
        let to = spans.iter().map(|s| s.1).max()?;
        let a = activity(f, &spans);
        let top_name = a
            .top_item
            .and_then(|(id, _)| f.names.get(&name_key('i', &id.to_string())).cloned())
            .unwrap_or_default();
        Some(Row {
            from,
            to,
            a,
            top_name,
        })
    }

    fn qualifies(&self) -> bool {
        self.a.played >= MIN_PLAYED
    }

    fn overlaps(&self, of: &Row) -> bool {
        self.from < of.to && self.to > of.from
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Peak {
    value: f64,
    from: i64,
    to: i64,
    level: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pace {
    key: &'static str,
    rate: f64,
    qualifies: bool,
    peers: usize,
    band: Option<[u32; 2]>,
    spread: Option<Spread>,
    best: Option<Peak>,
    record: Option<Peak>,
    is_best: bool,
    new_best: bool,
}

fn peak((r, v): (&Row, f64)) -> Peak {
    Peak {
        value: v,
        from: r.from,
        to: r.to,
        level: r.a.level[0],
    }
}

#[derive(Serialize)]
pub struct ItemRef {
    id: i64,
    name: String,
}

#[derive(Serialize)]
pub struct Record {
    key: &'static str,
    value: f64,
    at: i64,
    span: Option<[i64; 2]>,
    level: u32,
    item: Option<ItemRef>,
}

#[derive(Serialize)]
pub struct Skill {
    name: String,
    rank: i64,
    max: i64,
    kind: &'static str,
}

#[derive(Serialize)]
pub struct Rep {
    id: String,
    name: String,
    tier: u8,
    standing: i64,
    from: i64,
    to: i64,
}

#[derive(Serialize)]
pub struct TreeSpent {
    name: &'static str,
    spent: i64,
}

#[derive(Serialize)]
pub struct Node {
    tree: &'static str,
    name: String,
    rank: i64,
    max: i64,
}

#[derive(Serialize)]
pub struct LegacyGoal {
    earned: i64,
    max: i64,
    trees: Vec<TreeSpent>,
    nodes: Vec<Node>,
}

#[derive(Serialize, Default)]
pub struct Caps {
    cap: u32,
    skills: Vec<Skill>,
    reps: Vec<Rep>,
    legacy: Option<LegacyGoal>,
}

#[derive(Serialize)]
pub struct Latest {
    from: i64,
    to: i64,
    played: i64,
    paces: Vec<Pace>,
}

#[derive(Serialize)]
pub struct View<'a> {
    cap: u32,
    caps: &'a Caps,
    records: Vec<Record>,
    latest: Option<Latest>,
}

pub struct Goals {
    rows: Vec<Row>,
    cap: u32,
    peak: Option<(i64, i64)>,
    caps: Caps,
}

fn field(f: &[String], i: usize) -> &str {
    f.get(i).map_or("", String::as_str)
}

fn number(f: &[String], i: usize) -> i64 {
    field(f, i).parse().unwrap_or(0)
}

fn tree_name(id: &str) -> &'static str {
    match id {
        "1187" => "Professions",
        "1188" => "Adventure",
        _ => "Progression",
    }
}

#[derive(Default)]
struct Reading {
    cap: u32,
    skills: HashMap<String, (i64, i64)>,
    sheets: HashMap<String, (i64, i64)>,
    reps: HashMap<String, [i64; 4]>,
    nodes: HashMap<String, (String, String, i64, i64)>,
    balance: Vec<(i64, String, Vec<String>)>,
    legacy: Option<(i64, i64, [i64; 3])>,
}

impl Reading {
    fn feed(&mut self, f: &Facts, ms: i64, code: &str, fields: &[String]) {
        let name = |kind: char, id: &str| f.names.get(&name_key(kind, id)).cloned();
        match code {
            "S" => {
                if let Ok(cap @ 1..) = field(fields, 13).parse::<u32>() {
                    self.cap = cap;
                }
                self.balance.push((ms, code.into(), fields.to_vec()));
            }
            "M" | "ST" => self.balance.push((ms, code.into(), fields.to_vec())),
            "SL" => {
                if let Some(n) = name('k', field(fields, 0)) {
                    self.skills
                        .insert(n, (number(fields, 1), number(fields, 2)));
                }
            }
            "TS" if field(fields, 0) == "1" && !field(fields, 3).is_empty() => {
                self.sheets.insert(
                    field(fields, 2).to_string(),
                    (number(fields, 3), number(fields, 4)),
                );
            }
            "RF" => {
                let standing = field(fields, 2).parse().unwrap_or(i64::MIN);
                self.reps.insert(
                    field(fields, 0).to_string(),
                    [
                        number(fields, 1),
                        standing,
                        number(fields, 3),
                        number(fields, 4),
                    ],
                );
            }
            "LN" => {
                let spell = field(fields, 7);
                let label = match field(fields, 9) {
                    "" => name('s', spell).unwrap_or_else(|| format!("Node {}", field(fields, 1))),
                    own => own.to_string(),
                };
                self.nodes.insert(
                    field(fields, 1).to_string(),
                    (
                        field(fields, 0).to_string(),
                        label,
                        number(fields, 3),
                        number(fields, 6),
                    ),
                );
            }
            "LB" => {
                self.legacy = Some((
                    number(fields, 0),
                    number(fields, 4),
                    [number(fields, 5), number(fields, 6), number(fields, 7)],
                ));
            }
            _ => {}
        }
    }

    fn skills(&self) -> Vec<Skill> {
        let mut out: Vec<Skill> = self
            .skills
            .iter()
            .filter(|(n, (_, max))| *max > 1 && !n.starts_with("Language:"))
            .map(|(n, (rank, max))| Skill {
                name: n.clone(),
                rank: *rank,
                max: *max,
                kind: if PROFESSIONS.contains(&n.as_str()) {
                    "profession"
                } else {
                    "weapon"
                },
            })
            .collect();
        for (n, (rank, max)) in &self.sheets {
            if !self.skills.contains_key(n) && *max > 1 {
                out.push(Skill {
                    name: n.clone(),
                    rank: *rank,
                    max: *max,
                    kind: "profession",
                });
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    fn reps(&self, f: &Facts) -> Vec<Rep> {
        let mut out: Vec<Rep> = self
            .reps
            .iter()
            .map(|(id, [tier, standing, floor, ceiling])| {
                let tier = (*tier).clamp(1, 8) as usize;
                let (from, to) = match tier {
                    1..=3 if floor < ceiling => (*floor, *ceiling),
                    1..=3 => BELOW_NEUTRAL[tier - 1],
                    _ => (0, EXALTED),
                };
                Rep {
                    id: id.clone(),
                    name: f
                        .names
                        .get(&name_key('f', id))
                        .cloned()
                        .unwrap_or_else(|| format!("Faction {id}")),
                    tier: tier as u8,
                    standing: (*standing).clamp(from, to),
                    from,
                    to,
                }
            })
            .collect();
        out.sort_by(|a, b| {
            b.standing
                .cmp(&a.standing)
                .then_with(|| a.name.cmp(&b.name))
        });
        out
    }

    fn legacy(&self) -> Option<LegacyGoal> {
        let (earned, max, spent) = self.legacy?;
        let mut nodes: Vec<Node> = self
            .nodes
            .values()
            .filter(|(_, _, rank, _)| *rank > 0)
            .map(|(tree, name, rank, max)| Node {
                tree: tree_name(tree),
                name: name.clone(),
                rank: *rank,
                max: *max,
            })
            .collect();
        nodes.sort_by(|a, b| a.tree.cmp(b.tree).then_with(|| a.name.cmp(&b.name)));
        Some(LegacyGoal {
            earned,
            max,
            trees: ["1187", "1188", "1189"]
                .iter()
                .zip(spent)
                .map(|(id, spent)| TreeSpent {
                    name: tree_name(id),
                    spent,
                })
                .collect(),
            nodes,
        })
    }

    fn peak(&self) -> Option<(i64, i64)> {
        let evs: Vec<Ev> = self
            .balance
            .iter()
            .map(|(ms, code, fields)| (*ms, code.as_str(), fields.as_slice()))
            .collect();
        let mut best: Option<(i64, i64)> = None;
        for (ev, balance) in evs.iter().zip(balances(&evs)) {
            if let Some(b) = balance
                && best.is_none_or(|(_, top)| b > top)
            {
                best = Some((ev.0, b));
            }
        }
        best
    }
}

impl Goals {
    /// `groups` are the spans of each WoW run of the character.
    pub fn build(f: &Facts, groups: &[Vec<(i64, i64)>]) -> Goals {
        let mut reading = Reading::default();
        for r in f.events.between(0, i64::MAX) {
            reading.feed(f, r.ms, r.code, r.fields);
        }
        let mut rows: Vec<Row> = groups.iter().filter_map(|g| Row::of(f, g)).collect();
        rows.sort_by_key(|r| r.from);
        let caps = Caps {
            cap: reading.cap,
            skills: reading.skills(),
            reps: reading.reps(f),
            legacy: reading.legacy(),
        };
        Goals {
            cap: reading.cap,
            peak: reading.peak(),
            rows,
            caps,
        }
    }

    /// The row cut from exactly this run, when the run is one the goals were built from.
    pub fn row_of(&self, own: &Row) -> Option<&Row> {
        self.rows
            .iter()
            .find(|r| r.from == own.from && r.to == own.to && r.a.played == own.a.played)
    }

    fn near(&self, a: &Row, b: &Row) -> bool {
        let (x, y) = (a.a.level[0], b.a.level[0]);
        (self.cap > 0 && x >= self.cap && y >= self.cap) || x.abs_diff(y) <= BAND
    }

    fn peers<'a>(
        &'a self,
        m: &'a Metric,
        of: &'a Row,
    ) -> impl Iterator<Item = (&'a Row, f64)> + 'a {
        self.rows
            .iter()
            .filter(move |p| p.qualifies() && !p.overlaps(of) && (!m.banded || self.near(of, p)))
            .filter_map(move |p| m.of(&p.a, self.cap).map(|v| (p, v)))
    }

    fn band(&self, m: &Metric, row: &Row) -> Option<[u32; 2]> {
        let level = row.a.level[0];
        if !m.banded || level == 0 || (self.cap > 0 && level >= self.cap) {
            return None;
        }
        let hi = level + BAND;
        Some([
            level.saturating_sub(BAND).max(1),
            if self.cap > 0 { hi.min(self.cap) } else { hi },
        ])
    }

    fn top(&self, value: impl Fn(&Row) -> Option<f64>) -> Option<(&Row, f64)> {
        self.rows
            .iter()
            .filter(|r| r.qualifies())
            .filter_map(|r| value(r).filter(|v| *v > 0.0).map(|v| (r, v)))
            .max_by(|a, b| a.1.total_cmp(&b.1).then_with(|| b.0.from.cmp(&a.0.from)))
    }

    /// How `row` did on each rate it has, against the other runs of the same character that it does not overlap.
    /// `exact` is true when `row` is one of the runs the goals were built from; only then can it be a best.
    pub fn paces(&self, row: &Row, exact: bool) -> Vec<Pace> {
        METRICS
            .iter()
            .filter_map(|m| {
                let rate = m.of(&row.a, self.cap)?;
                let qualifies = row.qualifies();
                let peers: Vec<(&Row, f64)> = self.peers(m, row).collect();
                let enough = qualifies && peers.len() >= MIN_PEERS;
                let band = self.band(m, row);
                let best = peers
                    .iter()
                    .copied()
                    .max_by(|a, b| a.1.total_cmp(&b.1).then_with(|| b.0.from.cmp(&a.0.from)));
                let beaten = best
                    .map_or(0.0, |b| b.1)
                    .max(if qualifies { rate } else { 0.0 });
                let record = band
                    .and_then(|_| self.top(|r| m.of(&r.a, self.cap)))
                    .filter(|(_, v)| *v > beaten)
                    .map(peak);
                let earlier: Vec<f64> = peers
                    .iter()
                    .filter(|(r, _)| r.from < row.from)
                    .map(|p| p.1)
                    .collect();
                Some(Pace {
                    key: m.key,
                    rate,
                    qualifies,
                    peers: peers.len(),
                    band,
                    spread: enough.then(|| Spread::of(peers.iter().map(|p| p.1).collect(), rate)),
                    best: best.map(peak),
                    record,
                    is_best: exact && enough && peers.iter().all(|p| rate >= p.1),
                    new_best: exact
                        && qualifies
                        && earlier.len() >= MIN_PEERS
                        && earlier.iter().all(|v| rate > *v),
                })
            })
            .collect()
    }

    fn records(&self) -> Vec<Record> {
        let top = |value: &dyn Fn(&Row) -> Option<f64>| self.top(value);
        let made = |key, (r, v): (&Row, f64)| Record {
            key,
            value: v,
            at: r.from,
            span: Some([r.from, r.to]),
            level: r.a.level[0],
            item: None,
        };
        let mut out: Vec<Record> = METRICS
            .iter()
            .filter_map(|m| top(&|r| m.of(&r.a, self.cap)).map(|t| made(m.key, t)))
            .collect();
        for (key, value) in TOTALS {
            let Some(t) = top(&|r| Some(value(&r.a) as f64)) else {
                continue;
            };
            let mut record = made(key, t);
            if key == "item"
                && let Some((id, _)) = t.0.a.top_item
            {
                record.item = Some(ItemRef {
                    id,
                    name: t.0.top_name.clone(),
                });
            }
            out.push(record);
        }
        if let Some((ms, copper)) = self.peak {
            let held = self.rows.iter().find(|r| r.from <= ms && ms <= r.to);
            out.push(Record {
                key: "balance",
                value: copper as f64,
                at: ms,
                span: held.map(|r| [r.from, r.to]),
                level: held.map_or(0, |r| r.a.level[0]),
                item: None,
            });
        }
        out
    }

    pub fn view(&self) -> View<'_> {
        View {
            cap: self.cap,
            caps: &self.caps,
            records: self.records(),
            latest: self
                .rows
                .iter()
                .rev()
                .find(|r| r.qualifies())
                .map(|r| Latest {
                    from: r.from,
                    to: r.to,
                    played: r.a.played,
                    paces: self.paces(r, true),
                }),
        }
    }
}

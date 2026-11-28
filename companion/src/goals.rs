//! What a character is working toward and how well it does at it, from its own history only: the rates and records of
//! its WoW runs, and how far each skill, reputation and Legacy tree is from its ceiling.

use crate::events::name_key;
use crate::overview::{Ev, balances};
use crate::recap::{Activity, Facts, Spread, activity};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

const MIN_PLAYED: i64 = 900;
const MIN_PEERS: usize = 3;
const BAND: u32 = 2;
const EXALTED: i64 = 42_000;
const EXALTED_TIER: u8 = 8;
const RARE: u8 = 3;
const FARMED_TOP: usize = 3;
const TREND_RUNS: usize = 24;
const BRIEF_RUNS: usize = 40;
const RANK_COST: i64 = 1;
const FARMED_SHARE_PERCENT: i64 = 10;
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
    ranked: bool,
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

const METRICS: [Metric; 6] = [
    Metric {
        key: "xp_h",
        banded: true,
        ranked: true,
        value: |a, cap| (a.xp > 0 && (cap == 0 || a.level[0] < cap)).then(|| per_hour(a.xp, a)),
    },
    Metric {
        key: "gold_h",
        banded: true,
        ranked: true,
        value: |a, _| (a.net != 0 || a.coin != 0).then(|| per_hour(a.net, a)),
    },
    Metric {
        key: "kills_h",
        banded: true,
        ranked: true,
        value: |a, _| (a.kills > 0).then(|| per_hour(a.kills, a)),
    },
    Metric {
        key: "gathers_h",
        banded: false,
        ranked: true,
        value: |a, _| (a.gathers > 0).then(|| per_hour(a.gathers, a)),
    },
    Metric {
        key: "quests_h",
        banded: true,
        ranked: true,
        value: |a, _| (a.quests > 0).then(|| per_hour(a.quests, a)),
    },
    Metric {
        key: "deaths_h",
        banded: true,
        ranked: false,
        value: |a, _| Some(per_hour(a.deaths, a)),
    },
];

type Total = fn(&Activity) -> i64;

const TOTALS: [(&str, Total); 8] = [
    ("xp", |a| a.xp),
    ("coin", |a| a.coin),
    ("kills", |a| a.kills),
    ("gathers", |a| a.gathers),
    ("quests", |a| a.quests),
    ("loot", |a| a.items),
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

    fn farmed_items(&self) -> HashSet<i64> {
        let mut by_count: Vec<(i64, i64)> = self.a.looted.iter().map(|(id, n)| (*id, *n)).collect();
        by_count.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let items: i64 = by_count.iter().map(|(_, n)| n).sum();
        by_count
            .iter()
            .enumerate()
            .filter(|(i, (_, n))| {
                *n > 0 && (*i < FARMED_TOP || n * 100 >= items * FARMED_SHARE_PERCENT)
            })
            .map(|(_, (id, _))| *id)
            .collect()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Peak {
    pub value: f64,
    pub from: i64,
    pub to: i64,
    pub level: u32,
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
    unlearned: Option<Unlearned>,
    #[serde(skip)]
    ids: Vec<String>,
}

/// A skill the character no longer lists: its rank and maximum on its last line, the time of that line, and the time
/// of the first session baseline that lacked it.
#[derive(Serialize)]
pub struct Unlearned {
    last_held_ms: i64,
    rank: i64,
    max: i64,
    gone_by_ms: i64,
}

#[derive(Serialize, Clone)]
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
    available: i64,
    rank_cost: i64,
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
#[serde(rename_all = "camelCase")]
pub struct FarmPace {
    rate: f64,
    count: i64,
    from: i64,
    to: i64,
    played: i64,
    peers: usize,
    spread: Option<Spread>,
    best: Option<Peak>,
    is_best: bool,
}

#[derive(Serialize)]
pub struct Farmed {
    pub id: i64,
    pub name: String,
    pub quality: Option<u8>,
    pub total: i64,
    pub first: [i64; 2],
    pub record: Option<Peak>,
    pub pace: Option<FarmPace>,
}

struct ItemRun {
    row: usize,
    n: i64,
    farmed: bool,
}

struct ItemStat {
    total: i64,
    first: [i64; 2],
    runs: Vec<ItemRun>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapSummary {
    skills_known: usize,
    skills_maxed: usize,
    rep: Option<Rep>,
}

#[derive(Serialize)]
pub struct Latest {
    from: i64,
    to: i64,
    played: i64,
    paces: Vec<Pace>,
}

impl Latest {
    pub fn to(&self) -> i64 {
        self.to
    }
}

impl Record {
    pub fn key(&self) -> &str {
        self.key
    }

    pub fn value(&self) -> f64 {
        self.value
    }
}

#[derive(Serialize)]
pub struct View<'a> {
    cap: u32,
    caps: &'a Caps,
    records: Vec<Record>,
    latest: Option<Latest>,
    trend: Vec<Trend>,
    runs: Vec<Brief>,
}

#[derive(Serialize)]
pub struct Brief {
    from: i64,
    to: i64,
    played: i64,
    xp: i64,
    gold: i64,
    kills: i64,
    new_best: Vec<&'static str>,
}

#[derive(Serialize)]
pub struct Trend {
    key: &'static str,
    points: Vec<[f64; 2]>,
}

pub struct Goals {
    rows: Vec<Row>,
    cap: u32,
    peak: Option<(i64, i64)>,
    caps: Caps,
    items: HashMap<i64, ItemStat>,
    names: HashMap<i64, String>,
    quality: HashMap<i64, u8>,
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

struct Header {
    name: String,
    collapsed: bool,
    categories: HashSet<i64>,
}

#[derive(Default)]
struct Baseline {
    ms: i64,
    names: HashSet<String>,
    categories: HashSet<i64>,
    changed: HashSet<String>,
    headers: Vec<Header>,
}

#[derive(Default)]
struct Reading {
    cap: u32,
    skills: HashMap<String, (i64, i64)>,
    held_ms: HashMap<String, i64>,
    own_name: HashMap<String, String>,
    category: HashMap<String, i64>,
    ids: HashMap<String, HashSet<String>>,
    header_categories: HashMap<String, HashSet<i64>>,
    gone: HashMap<String, i64>,
    baseline: Option<Baseline>,
    sheets: HashMap<String, (i64, i64)>,
    reps: HashMap<String, [i64; 4]>,
    nodes: HashMap<String, (String, String, i64, i64)>,
    balance: Vec<(i64, String, Vec<String>)>,
    legacy: Option<(i64, i64, i64, [i64; 3])>,
    quality: HashMap<i64, u8>,
}

impl Reading {
    fn feed(&mut self, f: &Facts, ms: i64, code: &str, fields: &[String]) {
        let name = |kind: char, id: &str| f.names.get(&name_key(kind, id)).cloned();
        match code {
            "S" => {
                self.unlearn_skills_missing_from_baseline();
                if let Ok(cap @ 1..) = field(fields, 13).parse::<u32>() {
                    self.cap = cap;
                }
                self.balance.push((ms, code.into(), fields.to_vec()));
            }
            "M" | "ST" => self.balance.push((ms, code.into(), fields.to_vec())),
            "IQ" => {
                if let Ok(q) = field(fields, 1).parse::<u8>() {
                    self.quality.insert(number(fields, 0), q);
                }
            }
            "KN" => {
                let id = field(fields, 0);
                if !field(fields, 1).is_empty() {
                    self.own_name
                        .insert(id.to_string(), field(fields, 1).to_string());
                }
                if let Ok(category) = field(fields, 2).parse::<i64>() {
                    self.category.insert(id.to_string(), category);
                }
            }
            "SV" => {
                let baseline = self.baseline.get_or_insert_with(Baseline::default);
                baseline.headers.push(Header {
                    name: field(fields, 0).to_string(),
                    collapsed: field(fields, 1) != "0",
                    categories: field(fields, 2)
                        .split(',')
                        .filter_map(|c| c.parse().ok())
                        .collect(),
                });
            }
            "SL" => {
                let id = field(fields, 0);
                let known = self.own_name.get(id).cloned().or_else(|| name('k', id));
                if let Some(n) = known {
                    self.skills
                        .insert(n.clone(), (number(fields, 1), number(fields, 2)));
                    self.held_ms.insert(n.clone(), ms);
                    self.ids
                        .entry(n.clone())
                        .or_default()
                        .insert(id.to_string());
                    self.gone.remove(&n);
                    let baseline = self.baseline.get_or_insert_with(Baseline::default);
                    if field(fields, 4) == "s" {
                        if baseline.names.is_empty() {
                            baseline.ms = ms;
                        }
                        if let Some(category) = self.category.get(id) {
                            baseline.categories.insert(*category);
                        }
                        baseline.names.insert(n);
                    } else {
                        baseline.changed.insert(n);
                    }
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
                    number(fields, 1),
                    [number(fields, 5), number(fields, 6), number(fields, 7)],
                ));
            }
            _ => {}
        }
    }

    fn unlearn_skills_missing_from_baseline(&mut self) {
        let Some(baseline) = self.baseline.take() else {
            return;
        };
        for header in baseline
            .headers
            .iter()
            .filter(|h| !h.collapsed && !h.name.is_empty() && !h.categories.is_empty())
        {
            self.header_categories
                .entry(header.name.clone())
                .or_default()
                .extend(&header.categories);
        }
        if baseline.names.is_empty() {
            return;
        }
        let may_hide = |category: i64| {
            baseline.headers.iter().any(|h| {
                h.collapsed
                    && self
                        .header_categories
                        .get(&h.name)
                        .is_none_or(|known| known.contains(&category))
            })
        };
        let seen = |category: i64| {
            baseline.categories.contains(&category)
                || baseline
                    .headers
                    .iter()
                    .any(|h| !h.collapsed && h.categories.contains(&category))
                || (!baseline.headers.is_empty() && !may_hide(category))
        };
        let gone: Vec<String> = self
            .skills
            .keys()
            .filter(|name| !baseline.names.contains(*name) && !baseline.changed.contains(*name))
            .filter(|name| {
                self.ids
                    .get(*name)
                    .into_iter()
                    .flatten()
                    .filter_map(|id| self.category.get(id))
                    .any(|category| seen(*category))
            })
            .cloned()
            .collect();
        for name in gone {
            self.gone.entry(name).or_insert(baseline.ms);
        }
    }

    fn shown_name(&self, f: &Facts, key: &str) -> String {
        let lowest = self
            .ids
            .get(key)
            .and_then(|ids| ids.iter().min_by_key(|id| (id.len(), id.as_str())));
        lowest
            .and_then(|id| f.names.get(&name_key('k', id)).cloned())
            .unwrap_or_else(|| key.to_string())
    }

    fn skills(&self, f: &Facts) -> Vec<Skill> {
        let mut shown = HashSet::new();
        let mut out: Vec<Skill> = Vec::new();
        for (key, (rank, max)) in &self.skills {
            let n = self.shown_name(f, key);
            shown.insert(n.clone());
            if *max <= 1 || n.starts_with("Language:") {
                continue;
            }
            out.push(Skill {
                kind: if PROFESSIONS.contains(&n.as_str()) {
                    "profession"
                } else {
                    "weapon"
                },
                name: n,
                rank: *rank,
                max: *max,
                unlearned: self.gone.get(key).map(|gone_by| Unlearned {
                    last_held_ms: self.held_ms.get(key).copied().unwrap_or(*gone_by),
                    rank: *rank,
                    max: *max,
                    gone_by_ms: *gone_by,
                }),
                ids: self
                    .ids
                    .get(key)
                    .map(|ids| ids.iter().cloned().collect())
                    .unwrap_or_default(),
            });
        }
        for (n, (rank, max)) in &self.sheets {
            if !shown.contains(n) && *max > 1 {
                out.push(Skill {
                    name: n.clone(),
                    rank: *rank,
                    max: *max,
                    kind: "profession",
                    unlearned: None,
                    ids: Vec::new(),
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
        let (earned, max, available, spent) = self.legacy?;
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
            available,
            rank_cost: RANK_COST,
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
        reading.unlearn_skills_missing_from_baseline();
        let mut rows: Vec<Row> = groups.iter().filter_map(|g| Row::of(f, g)).collect();
        rows.sort_by_key(|r| r.from);
        let caps = Caps {
            cap: reading.cap,
            skills: reading.skills(f),
            reps: reading.reps(f),
            legacy: reading.legacy(),
        };
        let mut items: HashMap<i64, ItemStat> = HashMap::new();
        for (at, r) in rows.iter().enumerate() {
            let farmed = r.farmed_items();
            for (id, n) in &r.a.looted {
                let stat = items.entry(*id).or_insert(ItemStat {
                    total: 0,
                    first: [r.from, r.to],
                    runs: Vec::new(),
                });
                stat.total += n;
                if r.qualifies() && *n > 0 {
                    stat.runs.push(ItemRun {
                        row: at,
                        n: *n,
                        farmed: farmed.contains(id),
                    });
                }
            }
        }
        let names = items
            .keys()
            .filter_map(|id| {
                let name = f.names.get(&name_key('i', &id.to_string()))?;
                Some((*id, name.clone()))
            })
            .collect();
        Goals {
            cap: reading.cap,
            peak: reading.peak(),
            rows,
            caps,
            items,
            names,
            quality: reading.quality,
        }
    }

    pub fn farmed_totals(&self) -> impl Iterator<Item = (i64, i64)> + '_ {
        self.items.iter().map(|(id, s)| (*id, s.total))
    }

    pub fn rare_items(&self) -> impl Iterator<Item = i64> + '_ {
        self.items
            .keys()
            .copied()
            .filter(|id| self.quality.get(id).is_some_and(|q| *q >= RARE))
    }

    /// An item's total looted and its most in one qualifying run, without its pace.
    pub fn farmed_record(&self, id: i64) -> Option<(i64, Option<Peak>)> {
        let stat = self.items.get(&id)?;
        let record = stat
            .runs
            .iter()
            .max_by(|a, b| {
                a.n.cmp(&b.n)
                    .then_with(|| self.rows[b.row].from.cmp(&self.rows[a.row].from))
            })
            .map(|r| peak((&self.rows[r.row], r.n as f64)));
        Some((stat.total, record))
    }

    /// How much of the item was looted in the runs that farmed it; none when no run did.
    pub fn farming_count(&self, id: i64) -> Option<i64> {
        let stat = self.items.get(&id)?;
        let n: i64 = stat.runs.iter().filter(|r| r.farmed).map(|r| r.n).sum();
        (n > 0).then_some(n)
    }

    pub fn farmed(&self, id: i64) -> Option<Farmed> {
        let stat = self.items.get(&id)?;
        let (total, record) = self.farmed_record(id)?;
        let runs: Vec<(&Row, i64)> = stat
            .runs
            .iter()
            .filter(|r| r.farmed)
            .map(|r| (&self.rows[r.row], r.n))
            .collect();
        let pace = runs.last().map(|(row, n)| {
            let peers: Vec<(&Row, f64)> = runs
                .iter()
                .filter(|(p, _)| !p.overlaps(row))
                .map(|(p, k)| (*p, per_hour(*k, &p.a)))
                .collect();
            let now = per_hour(*n, &row.a);
            let enough = peers.len() >= MIN_PEERS;
            FarmPace {
                rate: now,
                count: *n,
                from: row.from,
                to: row.to,
                played: row.a.played,
                peers: peers.len(),
                spread: enough.then(|| Spread::of(peers.iter().map(|p| p.1).collect(), now)),
                best: peers
                    .iter()
                    .copied()
                    .max_by(|a, b| a.1.total_cmp(&b.1).then_with(|| b.0.from.cmp(&a.0.from)))
                    .map(peak),
                is_best: enough && peers.iter().all(|p| now >= p.1),
            }
        });
        Some(Farmed {
            id,
            name: self.names.get(&id).cloned().unwrap_or_default(),
            quality: self.quality.get(&id).copied(),
            total,
            first: stat.first,
            record,
            pace,
        })
    }

    fn learned(&self) -> impl Iterator<Item = &Skill> {
        self.caps.skills.iter().filter(|s| s.unlearned.is_none())
    }

    /// The ids of the skill lines of the skills the character has unlearned.
    pub fn unlearned_ids(&self) -> impl Iterator<Item = &str> {
        self.caps
            .skills
            .iter()
            .filter(|s| s.unlearned.is_some())
            .flat_map(|s| s.ids.iter().map(String::as_str))
    }

    pub fn summary(&self) -> CapSummary {
        let nearness = |r: &&Rep| {
            (
                r.tier,
                (r.standing - r.from) as f64 / (r.to - r.from).max(1) as f64,
            )
        };
        CapSummary {
            skills_known: self.learned().count(),
            skills_maxed: self.learned().filter(|s| s.rank >= s.max).count(),
            rep: self
                .caps
                .reps
                .iter()
                .filter(|r| r.tier < EXALTED_TIER)
                .max_by(|a, b| {
                    let (x, y) = (nearness(a), nearness(b));
                    x.0.cmp(&y.0).then(x.1.total_cmp(&y.1))
                })
                .cloned(),
        }
    }

    pub fn latest(&self) -> Option<Latest> {
        self.rows
            .iter()
            .rev()
            .find(|r| r.qualifies())
            .map(|r| Latest {
                from: r.from,
                to: r.to,
                played: r.a.played,
                paces: self.paces(r, true),
            })
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
                    .filter(|_| m.ranked)
                    .max_by(|a, b| a.1.total_cmp(&b.1).then_with(|| b.0.from.cmp(&a.0.from)));
                let beaten = best
                    .map_or(0.0, |b| b.1)
                    .max(if qualifies { rate } else { 0.0 });
                let record = band
                    .filter(|_| m.ranked)
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
                    is_best: m.ranked && exact && enough && peers.iter().all(|p| rate >= p.1),
                    new_best: m.ranked
                        && exact
                        && qualifies
                        && earlier.len() >= MIN_PEERS
                        && earlier.iter().all(|v| rate > *v),
                })
            })
            .collect()
    }

    pub fn records(&self) -> Vec<Record> {
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
            .filter(|m| m.ranked)
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
            latest: self.latest(),
            trend: self.trend(),
            runs: self.briefs(),
        }
    }

    fn briefs(&self) -> Vec<Brief> {
        self.rows
            .iter()
            .rev()
            .filter(|r| r.a.played > 0)
            .take(BRIEF_RUNS)
            .map(|r| Brief {
                from: r.from,
                to: r.to,
                played: r.a.played,
                xp: r.a.xp,
                gold: r.a.net,
                kills: r.a.kills,
                new_best: self
                    .paces(r, true)
                    .iter()
                    .filter(|p| p.new_best)
                    .map(|p| p.key)
                    .collect(),
            })
            .collect()
    }

    fn trend(&self) -> Vec<Trend> {
        let runs: Vec<&Row> = self.rows.iter().filter(|r| r.qualifies()).collect();
        let runs = &runs[runs.len().saturating_sub(TREND_RUNS)..];
        METRICS
            .iter()
            .filter_map(|m| {
                let points: Vec<[f64; 2]> = runs
                    .iter()
                    .filter_map(|r| m.of(&r.a, self.cap).map(|v| [r.from as f64, v]))
                    .collect();
                (points.len() >= 2).then_some(Trend { key: m.key, points })
            })
            .collect()
    }
}

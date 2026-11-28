//! A character's talents from its S, L, TG, TA, TY, TD, TQ and SN lines: the build by tab,
//! the order points were taken in, and respecs. Feeds the app's Talents view and talents.csv.

use crate::archive::Event;
use crate::art::art;
use crate::tables::csv;
use serde::Serialize;
use std::collections::{HashMap, HashSet, hash_map::Entry};

pub const CODES: [&str; 9] = ["S", "L", "TG", "TA", "TY", "TV", "TD", "TQ", "SN"];
pub const COLS: u32 = 4;
pub const ROWS: u32 = 7;
const CELL: f64 = 600.0;
const TYPO_FACTOR: i64 = 3;
const BURST_GAP_MS: i64 = 60_000;
const OFFER_WINDOW_MS: i64 = 600_000;
const RESET_WINDOW_MS: i64 = 120_000;
const RESPEC_POINTS: u32 = 2;

pub struct Line<'a> {
    pub ms: i64,
    pub code: &'a str,
    pub fields: &'a [String],
}

fn field<'a>(l: &Line<'a>, i: usize) -> &'a str {
    l.fields.get(i).map(String::as_str).unwrap_or("")
}

fn number(s: &str) -> Option<u32> {
    s.trim().parse().ok()
}

/// One TA line: a node's rank in a talent group.
#[derive(Serialize, Clone)]
pub struct Change {
    pub ms: i64,
    pub session: Option<u32>,
    pub level: Option<u32>,
    pub group: u32,
    pub node_id: String,
    pub entry_id: String,
    pub spell_id: String,
    pub spell: String,
    pub icon: Option<u32>,
    pub tab_id: String,
    pub tab: String,
    pub rank: u32,
    pub old_rank: Option<u32>,
    pub max_ranks: u32,
    /// `baseline`, `pick`, `refund`, `swap` (another entry of a choice node) or `unseen`
    /// (a baseline that differs from what the earlier lines left).
    pub kind: &'static str,
    /// The points the group holds after this line; empty on a baseline.
    pub point_no: Option<u32>,
}

/// The points a group already held when its first baseline was read.
#[derive(Serialize)]
pub struct Held {
    pub ms: i64,
    pub level: Option<u32>,
    pub group: u32,
    pub nodes: u32,
    pub points: u32,
}

#[derive(Serialize)]
pub struct Respec {
    pub ms: i64,
    pub level: Option<u32>,
    pub group: u32,
    pub points: u32,
    pub cost_copper: Option<i64>,
    /// `refund` for points taken back one by one, `reset` or `spec change` for the client's own.
    pub kind: &'static str,
}

#[derive(Serialize)]
pub struct Choice {
    pub spell_id: String,
    pub name: String,
    pub icon: Option<u32>,
}

#[derive(Serialize)]
pub struct BuildNode {
    pub node_id: String,
    pub spell_id: String,
    pub name: String,
    pub icon: Option<u32>,
    pub rank: u32,
    pub max_ranks: u32,
    pub col: Option<u32>,
    pub row: Option<u32>,
    pub to: Vec<String>,
    /// The entries of a choice node, the taken one first; empty on any other node.
    pub choices: Vec<Choice>,
}

#[derive(Serialize)]
pub struct TabBuild {
    pub id: String,
    pub name: String,
    pub spent: u32,
    pub available: Option<u32>,
    pub nodes: Vec<BuildNode>,
}

#[derive(Serialize)]
pub struct Build {
    pub ms: i64,
    pub group: u32,
    pub groups: u32,
    pub spec: String,
    pub unspent_class: Option<u32>,
    pub unspent_spec: Option<u32>,
    pub tabs: Vec<TabBuild>,
}

#[derive(Serialize, Default)]
pub struct Talents {
    pub build: Option<Build>,
    pub held: Vec<Held>,
    pub changes: Vec<Change>,
    pub respecs: Vec<Respec>,
    /// True when the addon wrote TD lines, so build nodes carry positions.
    pub layout: bool,
}

#[derive(Clone)]
struct Node {
    spell: String,
    tab: String,
    rank: u32,
    max: u32,
}

struct Burst {
    last: i64,
    respec: Respec,
}

fn is_respec(b: &Burst) -> bool {
    b.respec.points >= RESPEC_POINTS || b.respec.kind != "refund"
}

pub fn from_events(events: &[Event]) -> Talents {
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

type Place<'a> = (Option<i64>, Option<i64>, &'a str);

struct Variant<'a> {
    spell: &'a str,
    max: u32,
}

struct Slot<'a> {
    x: i64,
    y: i64,
    tab: &'a str,
    max: u32,
    entries: Vec<Variant<'a>>,
    edges: Vec<&'a str>,
}

struct Names<'a> {
    layouts: HashMap<&'a str, HashMap<&'a str, Slot<'a>>>,
    spells: HashMap<&'a str, &'a str>,
    tabs: HashMap<&'a str, &'a str>,
    places: HashMap<&'a str, Place<'a>>,
    shown: HashSet<&'a str>,
}

impl Names<'_> {
    fn shown_tab(&self, node: &str, tab: &str) -> String {
        if self.shown.is_empty() || self.shown.contains(tab) {
            return tab.to_string();
        }
        match self.places.get(node) {
            Some((_, _, t)) if !t.is_empty() => t.to_string(),
            _ => tab.to_string(),
        }
    }

    fn tab_name(&self, id: &str) -> String {
        match self.tabs.get(id) {
            Some(n) if !n.is_empty() => n.to_string(),
            _ if id.is_empty() => String::new(),
            _ => format!("Tab {id}"),
        }
    }

    fn spell(&self, id: &str) -> String {
        self.spells
            .get(id)
            .map(|s| s.to_string())
            .unwrap_or_default()
    }
}

fn names<'a>(lines: &[Line<'a>]) -> Names<'a> {
    let mut n = Names {
        layouts: HashMap::new(),
        spells: HashMap::new(),
        tabs: HashMap::new(),
        places: HashMap::new(),
        shown: HashSet::new(),
    };
    for l in lines {
        match l.code {
            "SN" => {
                n.spells.insert(field(l, 0), field(l, 1));
            }
            "TY" => {
                n.tabs.insert(field(l, 0), field(l, 1));
            }
            "TV" => {
                n.layouts.insert(field(l, 0), HashMap::new());
            }
            "TD" => {
                if let (Some(x), Some(y)) = (field(l, 2).parse().ok(), field(l, 3).parse().ok()) {
                    n.layouts.entry(field(l, 0)).or_default().insert(
                        field(l, 1),
                        Slot {
                            x,
                            y,
                            tab: field(l, 6),
                            max: number(field(l, 5)).unwrap_or(0),
                            entries: field(l, 7)
                                .split(',')
                                .filter_map(|e| {
                                    let mut it = e.split(':');
                                    let _entry = it.next().filter(|id| !id.is_empty())?;
                                    Some(Variant {
                                        spell: it.next().unwrap_or(""),
                                        max: it.next().and_then(number).unwrap_or(0),
                                    })
                                })
                                .collect(),
                            edges: field(l, 8)
                                .split(',')
                                .filter_map(|e| e.split(':').next().filter(|id| !id.is_empty()))
                                .collect(),
                        },
                    );
                }
                n.places.insert(
                    field(l, 1),
                    (
                        field(l, 2).parse().ok(),
                        field(l, 3).parse().ok(),
                        field(l, 6),
                    ),
                );
            }
            "TG" => {
                for t in field(l, 6).split(',') {
                    if let Some(id) = t.split(':').next().filter(|id| !id.is_empty()) {
                        n.shown.insert(id);
                    }
                }
            }
            _ => {}
        }
    }
    n
}

pub fn build<'a>(lines: impl IntoIterator<Item = Line<'a>>) -> Talents {
    let lines: Vec<Line<'a>> = lines.into_iter().collect();
    let names = names(&lines);
    let mut out = Talents {
        layout: !names.places.is_empty(),
        ..Talents::default()
    };

    let mut level: Option<u32> = None;
    let mut session: Option<u32> = None;
    let mut state: HashMap<u32, HashMap<String, Node>> = HashMap::new();
    let mut previous: HashMap<u32, HashMap<String, Node>> = HashMap::new();
    let mut based: HashMap<u32, Option<u32>> = HashMap::new();
    let mut first_based: HashMap<u32, (Option<u32>, usize)> = HashMap::new();
    let mut offer: Option<(i64, i64)> = None;
    let mut reset: Option<(i64, &'static str)> = None;
    let mut burst: Option<Burst> = None;
    let mut last_tg: Option<&Line> = None;

    for l in &lines {
        match l.code {
            "S" => {
                session = Some(session.map_or(0, |s| s + 1));
                level = number(field(l, 3)).or(level);
            }
            "L" => level = number(field(l, 0)).or(level),
            "TG" => last_tg = Some(l),
            "TQ" => match field(l, 0) {
                "p" => offer = field(l, 1).parse().ok().map(|c| (l.ms, c)),
                "r" if field(l, 1) != "1" => reset = Some((l.ms, "reset")),
                "i" if field(l, 1) != "1" => reset = Some((l.ms, "spec change")),
                _ => {}
            },
            "TA" => {
                let Some(group) = number(field(l, 0)) else {
                    continue;
                };
                let node_id = field(l, 1);
                let rank = number(field(l, 3)).unwrap_or(0);
                let old_rank = number(field(l, 4));
                let tab_id = names.shown_tab(node_id, field(l, 7));
                let baseline = field(l, 8) == "s";

                if baseline && based.get(&group) != Some(&session) {
                    based.insert(group, session);
                    previous.insert(group, state.remove(&group).unwrap_or_default());
                    if let Entry::Vacant(first) = first_based.entry(group) {
                        first.insert((session, out.held.len()));
                        out.held.push(Held {
                            ms: l.ms,
                            level,
                            group,
                            nodes: 0,
                            points: 0,
                        });
                    }
                }
                let kind = if baseline {
                    let known = previous.get(&group).and_then(|p| p.get(node_id));
                    let first = first_based.get(&group).is_some_and(|f| f.0 == session);
                    match known {
                        Some(k) if k.rank == rank => "baseline",
                        None if first => "baseline",
                        _ => "unseen",
                    }
                } else if rank > old_rank.unwrap_or(0) {
                    "pick"
                } else if rank < old_rank.unwrap_or(0) {
                    "refund"
                } else {
                    "swap"
                };
                if kind == "baseline"
                    && let Some(&(first_session, at)) = first_based.get(&group)
                    && first_session == session
                {
                    out.held[at].nodes += 1;
                    out.held[at].points += rank;
                }

                let nodes = state.entry(group).or_default();
                if rank == 0 {
                    nodes.remove(node_id);
                } else {
                    nodes.insert(
                        node_id.to_string(),
                        Node {
                            spell: field(l, 6).to_string(),
                            tab: tab_id.clone(),
                            rank,
                            max: number(field(l, 5)).unwrap_or(0),
                        },
                    );
                }
                let point_no = (!baseline).then(|| nodes.values().map(|n| n.rank).sum());

                if kind == "refund" {
                    let taken = old_rank.unwrap_or(0).saturating_sub(rank);
                    match &mut burst {
                        Some(b) if b.respec.group == group && l.ms - b.last <= BURST_GAP_MS => {
                            b.respec.points += taken;
                            b.last = l.ms;
                        }
                        _ => {
                            if let Some(b) = burst.take()
                                && is_respec(&b)
                            {
                                out.respecs.push(b.respec);
                            }
                            burst = Some(Burst {
                                last: l.ms,
                                respec: Respec {
                                    ms: l.ms,
                                    level,
                                    group,
                                    points: taken,
                                    cost_copper: offer
                                        .filter(|(at, _)| l.ms - at <= OFFER_WINDOW_MS)
                                        .map(|(_, cost)| cost),
                                    kind: match reset {
                                        Some((at, k)) if l.ms - at <= RESET_WINDOW_MS => k,
                                        _ => "refund",
                                    },
                                },
                            });
                        }
                    }
                }

                out.changes.push(Change {
                    ms: l.ms,
                    session,
                    level,
                    group,
                    node_id: node_id.to_string(),
                    entry_id: field(l, 2).to_string(),
                    spell_id: field(l, 6).to_string(),
                    spell: names.spell(field(l, 6)),
                    icon: art().icon(field(l, 6)),
                    tab: names.tab_name(&tab_id),
                    tab_id,
                    rank,
                    old_rank,
                    max_ranks: number(field(l, 5)).unwrap_or(0),
                    kind,
                    point_no,
                });
            }
            _ => {}
        }
    }
    if let Some(b) = burst
        && is_respec(&b)
    {
        out.respecs.push(b.respec);
    }
    out.build = last_tg.map(|tg| build_of(tg, &names, &mut state));
    out
}

fn tab_at(tabs: &mut Vec<TabBuild>, names: &Names, id: &str) -> usize {
    if let Some(at) = tabs.iter().position(|t| t.id == id) {
        return at;
    }
    tabs.push(TabBuild {
        id: id.to_string(),
        name: names.tab_name(id),
        spent: 0,
        available: None,
        nodes: Vec::new(),
    });
    tabs.len() - 1
}

fn median(mut values: Vec<i64>) -> i64 {
    values.sort_unstable();
    values.get(values.len() / 2).copied().unwrap_or(0)
}

fn read_back(v: i64, median: i64) -> i64 {
    if median > 0 && v > TYPO_FACTOR * median {
        v / 10
    } else {
        v
    }
}

fn cell(v: i64, origin: i64, last: u32) -> u32 {
    (((v - origin) as f64 / CELL).round().max(0.0) as u32).min(last - 1)
}

fn laid_out<'a>(
    layout: &HashMap<&'a str, Slot<'a>>,
    names: &Names,
    state: &HashMap<String, Node>,
    tabs: &mut Vec<TabBuild>,
    placed: &mut HashSet<&'a str>,
) {
    let med_x = median(layout.values().map(|s| s.x).collect());
    let med_y = median(layout.values().map(|s| s.y).collect());
    let at: HashMap<&str, (i64, i64)> = layout
        .iter()
        .map(|(id, s)| (*id, (read_back(s.x, med_x), read_back(s.y, med_y))))
        .collect();
    let top = at.values().map(|p| p.1).min().unwrap_or(0);
    let mut left: HashMap<&str, i64> = HashMap::new();
    for (id, s) in layout {
        let x = at[id].0;
        left.entry(s.tab)
            .and_modify(|l| *l = (*l).min(x))
            .or_insert(x);
    }
    let mut ids: Vec<&&str> = layout.keys().collect();
    ids.sort();
    for id in ids {
        let s = &layout[*id];
        let Some(first) = s.entries.first() else {
            continue;
        };
        if s.tab.is_empty() || (!names.shown.is_empty() && !names.shown.contains(s.tab)) {
            continue;
        }
        let taken = state.get(*id);
        let spell = taken.map_or(first.spell, |n| n.spell.as_str());
        let tab = tab_at(tabs, names, s.tab);
        let (x, y) = at[*id];
        let choices = if s.entries.len() > 1 {
            let mut all: Vec<&str> = s.entries.iter().map(|e| e.spell).collect();
            all.sort_by_key(|e| *e != spell);
            all.into_iter()
                .map(|e| Choice {
                    spell_id: e.to_string(),
                    name: names.spell(e),
                    icon: art().icon(e),
                })
                .collect()
        } else {
            Vec::new()
        };
        let mut to: Vec<String> = s
            .edges
            .iter()
            .filter(|t| layout.get(*t).is_some_and(|o| o.tab == s.tab))
            .map(|t| t.to_string())
            .collect();
        to.dedup();
        tabs[tab].nodes.push(BuildNode {
            node_id: id.to_string(),
            spell_id: spell.to_string(),
            name: names.spell(spell),
            icon: art().icon(spell),
            rank: taken.map_or(0, |n| n.rank),
            max_ranks: taken
                .map(|n| n.max)
                .filter(|m| *m > 0)
                .unwrap_or(first.max.max(s.max)),
            col: Some(cell(x, left[s.tab], COLS)),
            row: Some(cell(y, top, ROWS)),
            to,
            choices,
        });
        placed.insert(id);
    }
}

fn build_of(tg: &Line, names: &Names, state: &mut HashMap<u32, HashMap<String, Node>>) -> Build {
    let group = number(field(tg, 1)).unwrap_or(1);
    let nodes = state.remove(&group).unwrap_or_default();
    let mut tabs: Vec<TabBuild> = Vec::new();
    for t in field(tg, 6).split(',').filter(|t| !t.is_empty()) {
        let mut it = t.split(':');
        let id = it.next().unwrap_or("");
        let spent = it.next().and_then(number);
        let available = it.next().and_then(number);
        tabs.push(TabBuild {
            id: id.to_string(),
            name: names.tab_name(id),
            spent: spent.unwrap_or(0),
            available,
            nodes: Vec::new(),
        });
    }
    let mut placed: HashSet<&str> = HashSet::new();
    if let Some(layout) = names.layouts.get(field(tg, 5)) {
        laid_out(layout, names, &nodes, &mut tabs, &mut placed);
    }
    let mut ids: Vec<&String> = nodes.keys().collect();
    ids.sort();
    for id in ids.into_iter().filter(|id| !placed.contains(id.as_str())) {
        let n = &nodes[id];
        let at = tab_at(&mut tabs, names, &n.tab);
        tabs[at].nodes.push(BuildNode {
            node_id: id.clone(),
            spell_id: n.spell.clone(),
            name: names.spell(&n.spell),
            icon: art().icon(&n.spell),
            rank: n.rank,
            max_ranks: n.max,
            col: None,
            row: None,
            to: Vec::new(),
            choices: Vec::new(),
        });
    }
    for t in &mut tabs {
        t.nodes.sort_by_key(|n| {
            (
                n.row.unwrap_or(u32::MAX),
                n.col.unwrap_or(u32::MAX),
                n.node_id.clone(),
            )
        });
        if t.spent == 0 {
            t.spent = t.nodes.iter().map(|n| n.rank).sum();
        }
    }
    let left = |t: &TabBuild| {
        t.nodes
            .iter()
            .filter_map(|n| names.layouts.get(field(tg, 5))?.get(n.node_id.as_str()))
            .map(|s| s.x)
            .min()
    };
    if tabs.iter().all(|t| left(t).is_some()) {
        tabs.sort_by_key(left);
    }
    Build {
        ms: tg.ms,
        group,
        groups: number(field(tg, 2)).unwrap_or(1),
        spec: field(tg, 4).to_string(),
        unspent_class: number(field(tg, 7)),
        unspent_spec: number(field(tg, 8)),
        tabs,
    }
}

impl Talents {
    pub fn csv(&self) -> String {
        let mut out = String::from(
            "ms,session,level,group,node_id,entry_id,spell_id,spell,tab_id,tab,rank,old_rank,max_ranks,kind,point_no\n",
        );
        let opt = |v: Option<u32>| v.map(|n| n.to_string()).unwrap_or_default();
        for c in &self.changes {
            out.push_str(&csv(&[
                c.ms.to_string(),
                opt(c.session),
                opt(c.level),
                c.group.to_string(),
                c.node_id.clone(),
                c.entry_id.clone(),
                c.spell_id.clone(),
                c.spell.clone(),
                c.tab_id.clone(),
                c.tab.clone(),
                c.rank.to_string(),
                opt(c.old_rank),
                c.max_ranks.to_string(),
                c.kind.to_string(),
                opt(c.point_no),
            ]));
        }
        out
    }
}

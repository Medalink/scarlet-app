//! The Characters page: every recorded character's overview from one pass over the store, the
//! game accounts they sit in, and what only shows across characters: money moved between them,
//! professions and time played together.

use crate::commands::{state, with_store};
use crate::store::Store;
use scarlet::events::name_key;
use scarlet::overview::{self, Overview};
use serde::Serialize;
use std::collections::HashMap;
use tauri::AppHandle;

const PROFESSIONS: [&str; 15] = [
    "Alchemy",
    "Archaeology",
    "Blacksmithing",
    "Cooking",
    "Enchanting",
    "Engineering",
    "First Aid",
    "Fishing",
    "Herbalism",
    "Inscription",
    "Jewelcrafting",
    "Leatherworking",
    "Mining",
    "Skinning",
    "Tailoring",
];

const MIN_TOGETHER_MS: i64 = 60_000;
const TRADE_WINDOW_MS: i64 = 120_000;
const MAIL_WINDOW_MS: i64 = 31 * 86_400_000;
const MAX_TRANSFERS: usize = 200;

#[derive(Serialize)]
pub struct Skill {
    name: String,
    rank: i64,
    max: i64,
}

#[derive(Serialize)]
pub struct Reach {
    level: i64,
    date: String,
}

#[derive(Serialize)]
pub struct RosterChar {
    name: String,
    class: String,
    race: String,
    level: i64,
    xp: i64,
    xp_max: i64,
    zone: String,
    played: i64,
    money: Option<i64>,
    through: i64,
    cap: i64,
    reach: Option<Reach>,
    /// `[start, played, earned, spent]` per session, newest first; earned and spent leave out
    /// money moved between the player's own characters.
    sessions: Vec<[i64; 4]>,
    /// `[end, balance]` per session that read one, oldest first.
    balance: Vec<[i64; 2]>,
    /// `[level, seconds]` for every level with time on it.
    levels: Vec<[i64; 2]>,
    professions: Vec<Skill>,
    sent: i64,
    received: i64,
}

#[derive(Serialize)]
pub struct Group {
    account: String,
    characters: Vec<String>,
}

#[derive(Serialize)]
pub struct Transfer {
    ms: i64,
    from: String,
    to: String,
    copper: i64,
    via: &'static str,
}

#[derive(Serialize)]
pub struct Pair {
    a: String,
    b: String,
    together: i64,
    grouped: i64,
    sessions: i64,
    last: i64,
}

#[derive(Serialize)]
pub struct Roster {
    chars: Vec<RosterChar>,
    groups: Vec<Group>,
    transfers: Vec<Transfer>,
    transfers_total: usize,
    pairs: Vec<Pair>,
}

struct Lookup {
    full: HashMap<String, usize>,
    first: HashMap<String, Option<usize>>,
}

fn plain(s: &str) -> String {
    s.replace('-', " ").trim().to_lowercase()
}

impl Lookup {
    fn new(names: &[String]) -> Self {
        let mut full = HashMap::new();
        let mut first: HashMap<String, Option<usize>> = HashMap::new();
        for (i, n) in names.iter().enumerate() {
            let p = plain(n);
            let head = p.split(' ').next().unwrap_or("").to_string();
            full.insert(p, i);
            first
                .entry(head)
                .and_modify(|v| *v = None)
                .or_insert(Some(i));
        }
        Lookup { full, first }
    }

    fn character_named(&self, who: &str) -> Option<usize> {
        let p = plain(who);
        self.full
            .get(&p)
            .copied()
            .or_else(|| self.first.get(&p).copied().flatten())
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Side {
    Sent,
    Received,
}

struct Move {
    ms: i64,
    from: usize,
    to: usize,
    copper: i64,
    via: &'static str,
    side: Side,
}

fn field(f: &[String], i: usize) -> &str {
    f.get(i).map_or("", String::as_str)
}

fn copper(f: &[String], i: usize) -> i64 {
    field(f, i).parse().unwrap_or(0)
}

fn own_transfers(moves: Vec<Move>, names: &[String]) -> Vec<Transfer> {
    let mut out: Vec<Move> = Vec::new();
    let mut sends: Vec<(Move, bool)> = Vec::new();
    let mut receipts: Vec<Move> = Vec::new();
    for m in moves {
        if m.side == Side::Sent {
            sends.push((m, false));
        } else {
            receipts.push(m);
        }
    }
    for r in receipts {
        let hit = sends.iter_mut().find(|(s, used)| {
            !*used
                && s.via == r.via
                && s.from == r.from
                && s.to == r.to
                && s.copper == r.copper
                && if r.via == "trade" {
                    (s.ms - r.ms).abs() <= TRADE_WINDOW_MS
                } else {
                    r.ms >= s.ms - 60_000 && r.ms - s.ms <= MAIL_WINDOW_MS
                }
        });
        match hit {
            Some((_, used)) => *used = true,
            None => out.push(r),
        }
    }
    out.extend(sends.into_iter().map(|(s, _)| s));
    out.sort_by_key(|m| std::cmp::Reverse(m.ms));
    out.into_iter()
        .map(|m| Transfer {
            ms: m.ms,
            from: names[m.from].clone(),
            to: names[m.to].clone(),
            copper: m.copper,
            via: m.via,
        })
        .collect()
}

fn skills(latest: HashMap<u32, (i64, i64)>, st: &Store) -> Vec<Skill> {
    let mut best: HashMap<String, (i64, i64)> = HashMap::new();
    for (id, (rank, max)) in latest {
        let Some(name) = st.names.get(&name_key('k', &id.to_string())) else {
            continue;
        };
        if !PROFESSIONS.contains(&name.as_str()) {
            continue;
        }
        let e = best.entry(name.clone()).or_insert((0, 0));
        if rank > e.0 {
            *e = (rank, max);
        }
    }
    let mut out: Vec<Skill> = best
        .into_iter()
        .filter(|(_, (rank, _))| *rank > 0)
        .map(|(name, (rank, max))| Skill {
            name,
            rank,
            max: max.max(rank),
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn together_in(a: &[(i64, i64)], b: &[(i64, i64)]) -> (i64, i64, i64) {
    let (mut ms, mut n, mut last) = (0, 0, 0);
    for &(a0, a1) in a {
        for &(b0, b1) in b {
            if b0 >= a1 {
                break;
            }
            let (lo, hi) = (a0.max(b0), a1.min(b1));
            if hi - lo >= MIN_TOGETHER_MS {
                ms += hi - lo;
                n += 1;
                last = last.max(hi);
            }
        }
    }
    ((ms + 500) / 1000, n, last)
}

#[tauri::command(async)]
pub fn roster(app: AppHandle) -> Roster {
    let folders = state(&app).settings.lock().unwrap().folders();
    let accounts: Vec<(String, Vec<String>)> = folders
        .iter()
        .flat_map(|f| scarlet::export::accounts(f))
        .map(|a| (a.name, a.characters))
        .collect();
    with_store(&app, |st| build(st, accounts))
}

fn build(st: &Store, accounts: Vec<(String, Vec<String>)>) -> Roster {
    let look = Lookup::new(&st.chars);
    let n = st.chars.len();
    let mut events: Vec<Vec<overview::Ev>> = (0..n).map(|_| Vec::new()).collect();
    let mut skill_rows: Vec<HashMap<u32, (i64, i64)>> = (0..n).map(|_| HashMap::new()).collect();
    let mut moves: Vec<Move> = Vec::new();
    for r in &st.events {
        let c = r.ch as usize;
        events[c].push((r.ms, r.code.as_str(), r.fields.as_slice()));
        let f = r.fields.as_slice();
        match r.code.as_str() {
            "SL" => {
                if let Ok(id) = field(f, 0).parse::<u32>() {
                    skill_rows[c].insert(id, (copper(f, 1), copper(f, 2)));
                }
            }
            "ML" => {
                let Some(other) = look.character_named(field(f, 1)).filter(|o| *o != c) else {
                    continue;
                };
                let money = copper(f, 2);
                if money <= 0 {
                    continue;
                }
                match field(f, 0) {
                    "s" => moves.push(Move {
                        ms: r.ms,
                        from: c,
                        to: other,
                        copper: money,
                        via: "mail",
                        side: Side::Sent,
                    }),
                    "m" => moves.push(Move {
                        ms: r.ms,
                        from: other,
                        to: c,
                        copper: money,
                        via: "mail",
                        side: Side::Received,
                    }),
                    _ => {}
                }
            }
            "TR" if matches!(field(f, 2), "1" | "2") => {
                let Some(other) = look.character_named(field(f, 1)).filter(|o| *o != c) else {
                    continue;
                };
                let (gave, got) = (copper(f, 3), copper(f, 4));
                if gave > 0 {
                    moves.push(Move {
                        ms: r.ms,
                        from: c,
                        to: other,
                        copper: gave,
                        via: "trade",
                        side: Side::Sent,
                    });
                }
                if got > 0 {
                    moves.push(Move {
                        ms: r.ms,
                        from: other,
                        to: c,
                        copper: got,
                        via: "trade",
                        side: Side::Received,
                    });
                }
            }
            _ => {}
        }
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let overviews: Vec<Overview> = events
        .into_iter()
        .zip(&st.chars)
        .map(|(evs, name)| overview::build(name, evs))
        .collect();

    let moved = own_transfers(moves, &st.chars);
    let mut sent = vec![0i64; n];
    let mut received = vec![0i64; n];
    for t in &moved {
        if let (Some(a), Some(b)) = (look.character_named(&t.from), look.character_named(&t.to)) {
            sent[a] += t.copper;
            received[b] += t.copper;
        }
    }

    let mut pairs = Vec::new();
    let spans: Vec<Vec<(i64, i64)>> = (0..n as u32)
        .map(|c| {
            let mut v: Vec<(i64, i64)> = st
                .sessions
                .iter()
                .filter(|s| s.ch == c)
                .map(|s| (s.start, s.end))
                .collect();
            v.sort_unstable();
            v
        })
        .collect();
    for a in 0..n {
        for b in a + 1..n {
            let (together, sessions, last) = together_in(&spans[a], &spans[b]);
            let said = |x: usize, y: usize| {
                overviews[x]
                    .mates
                    .iter()
                    .find(|m| look.character_named(&m.name) == Some(y))
                    .map_or(0, |m| m.p)
            };
            let grouped = said(a, b).max(said(b, a));
            if together > 0 || grouped > 0 {
                pairs.push(Pair {
                    a: st.chars[a].clone(),
                    b: st.chars[b].clone(),
                    together,
                    grouped,
                    sessions,
                    last: last / 1000,
                });
            }
        }
    }
    pairs.sort_by(|x, y| {
        y.together
            .cmp(&x.together)
            .then(y.grouped.cmp(&x.grouped))
            .then(x.a.cmp(&y.a))
    });

    let chars: Vec<RosterChar> = overviews
        .into_iter()
        .zip(&st.chars)
        .zip(skill_rows)
        .enumerate()
        .map(|(i, ((ov, name), rows))| {
            let reach = ov.eta(now).targets.pop().map(|t| Reach {
                level: t.level,
                date: t.date,
            });
            let cap = ov.cap.as_ref().map_or(0, |c| c.level);
            RosterChar {
                name: name.clone(),
                class: ov.class,
                race: ov.race,
                level: ov.level,
                xp: ov.xp,
                xp_max: ov.xp_max,
                zone: ov.zone,
                played: ov.played,
                money: ov.money,
                through: ov.through,
                cap,
                reach,
                balance: ov
                    .sessions
                    .iter()
                    .rev()
                    .filter_map(|s| s.m.map(|m| [s.t + s.p, m]))
                    .collect(),
                sessions: ov.sessions.iter().map(|s| [s.t, s.p, s.g, s.s]).collect(),
                levels: ov
                    .levels
                    .iter()
                    .filter(|(_, l)| l.share.p > 0)
                    .map(|(lv, l)| [i64::from(*lv), l.share.p])
                    .collect(),
                professions: skills(rows, st),
                sent: sent[i],
                received: received[i],
            }
        })
        .collect();

    let mut groups: Vec<Group> = Vec::new();
    let mut placed = vec![false; n];
    for (account, names) in accounts {
        let here: Vec<String> = names
            .into_iter()
            .filter(|name| {
                st.char_index(name)
                    .is_some_and(|i| !std::mem::replace(&mut placed[i as usize], true))
            })
            .collect();
        if !here.is_empty() {
            groups.push(Group {
                account,
                characters: here,
            });
        }
    }
    let rest: Vec<String> = (0..n)
        .filter(|i| !placed[*i])
        .map(|i| st.chars[i].clone())
        .collect();
    if !rest.is_empty() {
        groups.push(Group {
            account: String::new(),
            characters: rest,
        });
    }

    let transfers_total = moved.len();
    Roster {
        chars,
        groups,
        transfers: moved.into_iter().take(MAX_TRANSFERS).collect(),
        transfers_total,
        pairs,
    }
}

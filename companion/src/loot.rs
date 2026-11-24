//! Loot attribution: the current loot rule carried as state, drop rates per
//! creature and item split by that rule, and what each character in the group
//! received and rolled, per docs/FORMAT.md ("Loot rule" and "Companion output").
//! One `LootBook` reads one character's events in time order; the export and
//! the desktop app both build their tables from it.

use crate::events::name_key;
use crate::tables::csv;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Two roll lines for the same player and item more than this far apart are
/// two different rolls (the history and the chat messages of one roll land
/// within seconds of each other).
const ROLL_WINDOW_MS: i64 = 60_000;
/// The roll history repeats a win the chat already gave (its rows, then the win)
/// within the same moment; events this close to a closed roll still belong to it.
const ROLL_ECHO_MS: i64 = 2_000;

/// The split key everywhere: the loot method, the quality threshold and the
/// group size. `method` is `solo`, the client's method name in lower case, or
/// `unknown` for a stretch before any `LM` line (older logs).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Rule {
    pub method: String,
    pub threshold: String,
    pub size: u32,
}

impl Rule {
    fn unknown() -> Self {
        Rule {
            method: "unknown".into(),
            threshold: String::new(),
            size: 0,
        }
    }

    fn solo() -> Self {
        Rule {
            method: "solo".into(),
            threshold: String::new(),
            size: 1,
        }
    }
}

struct Roll {
    last: i64,
    rule: Rule,
    joined: bool,
    passed: bool,
    won: bool,
    /// A win or loss closed it: the next roll by this player on this item is a new one.
    closed: bool,
}

#[derive(Default, Clone, Copy)]
struct RollSeen {
    joined: bool,
    passed: bool,
    won: bool,
    outcome: bool,
}

#[derive(Default, Clone, Copy)]
struct RollTotals {
    won: u64,
    lost: u64,
    passed: u64,
}

impl RollTotals {
    fn add(&mut self, r: &Roll) {
        if r.won {
            self.won += 1;
        } else if r.joined {
            self.lost += 1;
        } else if r.passed {
            self.passed += 1;
        }
    }
}

pub struct LootBook {
    me: String,
    rule: Rule,
    master: String,
    /// (npcID, rule) -> kills.
    kills: BTreeMap<(String, Rule), u64>,
    /// (npcID, rule) -> corpses looted by you or reported by a group member's
    /// Scarlet, with a known item list.
    looted: BTreeMap<(String, Rule), u64>,
    /// (npcID, itemID, rule) -> (corpses it dropped from, quantity).
    drops: BTreeMap<(String, String, Rule), (u64, u64)>,
    /// (player, rule, itemID) -> (receipts, quantity).
    receipts: BTreeMap<(String, Rule, String), (u64, u64)>,
    /// itemID -> quality, from IQ lines.
    quality: HashMap<String, u8>,
    /// (player, itemID) -> the roll in progress.
    rolls: HashMap<(String, String), Roll>,
    roll_totals: BTreeMap<(String, Rule), RollTotals>,
}

pub struct DropRow {
    pub npc: String,
    pub npc_name: String,
    pub item: String,
    pub item_name: String,
    pub rule: Rule,
    pub kills: u64,
    pub looted: u64,
    pub drops: u64,
    pub quantity: u64,
}

impl DropRow {
    /// Corpses it dropped from per corpse looted (by you or reported by a
    /// group member's Scarlet).
    pub fn rate(&self) -> Option<f64> {
        (self.looted > 0).then(|| self.drops as f64 / self.looted as f64)
    }
}

pub struct AttrRow {
    pub who: String,
    pub rule: Rule,
    pub items: u64,
    pub quantity: u64,
    /// Receipts by quality: poor, common, uncommon, rare, epic, legendary and
    /// above, then unknown.
    pub quality: [u64; 7],
    pub rolls_won: u64,
    pub rolls_lost: u64,
    pub rolls_passed: u64,
}

fn num(v: Option<&String>) -> u64 {
    v.and_then(|s| s.parse().ok()).unwrap_or(0)
}

impl LootBook {
    pub fn new(me: &str) -> Self {
        LootBook {
            me: me.to_string(),
            rule: Rule::unknown(),
            master: String::new(),
            kills: BTreeMap::new(),
            looted: BTreeMap::new(),
            drops: BTreeMap::new(),
            receipts: BTreeMap::new(),
            quality: HashMap::new(),
            rolls: HashMap::new(),
            roll_totals: BTreeMap::new(),
        }
    }

    /// The loot rule in force after the last event applied.
    pub fn rule(&self) -> &Rule {
        &self.rule
    }

    /// The master looter's Name-Surname while the method is `masterlooter`.
    pub fn master(&self) -> &str {
        &self.master
    }

    fn receipt(&mut self, who: &str, item: &str, qty: u64) {
        if item.is_empty() {
            return;
        }
        // Forever's loot messages show "Name Surname"; characters are keyed Name-Surname.
        let who = who.split_whitespace().collect::<Vec<_>>().join("-");
        let e = self
            .receipts
            .entry((who, self.rule.clone(), item.to_string()))
            .or_default();
        e.0 += 1;
        e.1 += qty.max(1);
    }

    fn roll(&mut self, ms: i64, who: &str, item: &str, seen: RollSeen) {
        if who.is_empty() || item.is_empty() {
            return;
        }
        let key = (who.to_string(), item.to_string());
        if self.rolls.get(&key).is_some_and(|r| {
            ms - r.last > ROLL_WINDOW_MS
                || (r.closed && !seen.outcome && ms - r.last > ROLL_ECHO_MS)
        }) && let Some(old) = self.rolls.remove(&key)
        {
            self.roll_totals
                .entry((key.0.clone(), old.rule.clone()))
                .or_default()
                .add(&old);
        }
        let rule = self.rule.clone();
        let r = self.rolls.entry(key).or_insert(Roll {
            last: ms,
            rule,
            joined: false,
            passed: false,
            won: false,
            closed: false,
        });
        r.last = ms;
        r.joined |= seen.joined;
        r.passed |= seen.passed;
        r.won |= seen.won;
        r.closed |= seen.outcome;
    }

    fn window(&mut self, npc: &str, items: &str) {
        let npc = npc.to_string();
        let mut counts: BTreeMap<&str, u64> = BTreeMap::new();
        for tok in items.split(',') {
            let (what, qty) = tok.split_once('x').unwrap_or((tok, ""));
            if what.starts_with(|c: char| c.is_ascii_digit()) {
                *counts.entry(what).or_default() += qty.parse().unwrap_or(1);
            }
        }
        *self
            .looted
            .entry((npc.clone(), self.rule.clone()))
            .or_default() += 1;
        for (item, qty) in counts {
            let e = self
                .drops
                .entry((npc.clone(), item.to_string(), self.rule.clone()))
                .or_default();
            e.0 += 1;
            e.1 += qty;
        }
    }

    /// Apply one event. Codes this module does not read are ignored.
    pub fn apply(&mut self, ms: i64, code: &str, f: &[String]) {
        let g = |i: usize| f.get(i).map(String::as_str).unwrap_or("");
        match code {
            "S" => {
                self.rule = Rule::unknown();
                self.master.clear();
            }
            "GP" => {
                if g(0) == "1" {
                    if self.rule.method == "solo" {
                        self.rule.method = "unknown".into();
                        self.rule.threshold.clear();
                    }
                    self.rule.size = g(1).parse().unwrap_or(self.rule.size);
                } else {
                    self.rule = Rule::solo();
                    self.master.clear();
                }
            }
            "LM" => {
                self.rule = Rule {
                    method: if g(0).is_empty() { "unknown" } else { g(0) }.to_string(),
                    threshold: g(1).to_string(),
                    size: g(3).parse().unwrap_or(0),
                };
                self.master = g(2).to_string();
            }
            "K" => {
                *self
                    .kills
                    .entry((g(0).to_string(), self.rule.clone()))
                    .or_default() += 1;
            }
            // A second read of the same corpse (skinning, reopening) is not a new drop.
            "LO" if g(0) == "c" && !g(3).is_empty() && g(4) != "1" => self.window(g(1), g(3)),
            "LW" if g(1) == "c" && !g(4).is_empty() && g(5) != "1" => self.window(g(2), g(4)),
            "O" => {
                let me = self.me.clone();
                self.receipt(&me, g(0), num(f.get(1)));
            }
            "LT" => self.receipt(g(1), g(2), num(f.get(3))),
            "IQ" => {
                if let Ok(q) = g(1).parse::<u8>() {
                    self.quality.insert(g(0).to_string(), q);
                }
            }
            // Your own rolls: RL is what you chose, RW w and RW l the outcome.
            "RL" => {
                let me = self.me.clone();
                let pass = g(0) == "0";
                let seen = RollSeen {
                    joined: !pass,
                    passed: pass,
                    ..RollSeen::default()
                };
                self.roll(ms, &me, g(1), seen);
            }
            "RW" if g(0) == "w" || g(0) == "l" => {
                let me = self.me.clone();
                let seen = RollSeen {
                    joined: true,
                    won: g(0) == "w",
                    outcome: true,
                    ..RollSeen::default()
                };
                self.roll(ms, &me, g(1), seen);
            }
            // Other players' rolls, from the chat messages and the roll history.
            "LD" => match g(0) {
                "r" | "s" => {
                    let seen = RollSeen {
                        joined: true,
                        ..RollSeen::default()
                    };
                    self.roll(ms, g(1), g(2), seen);
                }
                "p" => {
                    let seen = RollSeen {
                        passed: true,
                        ..RollSeen::default()
                    };
                    self.roll(ms, g(1), g(2), seen);
                }
                "w" => {
                    let seen = RollSeen {
                        joined: true,
                        won: true,
                        outcome: true,
                        ..RollSeen::default()
                    };
                    self.roll(ms, g(1), g(2), seen);
                }
                _ => {}
            },
            _ => {}
        }
    }

    /// The drop-rate rows. Every (creature, item) pair that dropped at least
    /// once gets a row for every rule that creature was killed or looted under,
    /// so a rule with no drop shows as zero instead of missing.
    pub fn drop_rows(&self, names: &HashMap<String, String>) -> Vec<DropRow> {
        let name = |kind, id: &str| names.get(&name_key(kind, id)).cloned().unwrap_or_default();
        let mut pairs: BTreeSet<(&String, &String)> = BTreeSet::new();
        for (npc, item, _) in self.drops.keys() {
            pairs.insert((npc, item));
        }
        let mut rules: BTreeMap<&String, BTreeSet<&Rule>> = BTreeMap::new();
        for (npc, rule) in self.kills.keys().chain(self.looted.keys()) {
            rules.entry(npc).or_default().insert(rule);
        }
        let mut out = Vec::new();
        for (npc, item) in pairs {
            for rule in rules.get(npc).into_iter().flatten() {
                let looted = self
                    .looted
                    .get(&(npc.clone(), (*rule).clone()))
                    .copied()
                    .unwrap_or(0);
                if looted == 0 {
                    continue;
                }
                let (drops, quantity) = self
                    .drops
                    .get(&(npc.clone(), item.clone(), (*rule).clone()))
                    .copied()
                    .unwrap_or((0, 0));
                out.push(DropRow {
                    npc: npc.clone(),
                    npc_name: name('n', npc),
                    item: item.clone(),
                    item_name: name('i', item),
                    rule: (*rule).clone(),
                    kills: self
                        .kills
                        .get(&(npc.clone(), (*rule).clone()))
                        .copied()
                        .unwrap_or(0),
                    looted,
                    drops,
                    quantity,
                });
            }
        }
        out
    }

    /// Per player and rule: items received by quality, and roll results.
    /// Rolls still open count as they stand.
    pub fn attribution_rows(&self) -> Vec<AttrRow> {
        let mut totals = self.roll_totals.clone();
        for ((who, _), r) in &self.rolls {
            totals
                .entry((who.clone(), r.rule.clone()))
                .or_default()
                .add(r);
        }
        let mut rows: BTreeMap<(String, Rule), AttrRow> = BTreeMap::new();
        let blank = |who: &String, rule: &Rule| AttrRow {
            who: who.clone(),
            rule: rule.clone(),
            items: 0,
            quantity: 0,
            quality: [0; 7],
            rolls_won: 0,
            rolls_lost: 0,
            rolls_passed: 0,
        };
        for ((who, rule, item), (count, qty)) in &self.receipts {
            let row = rows
                .entry((who.clone(), rule.clone()))
                .or_insert_with(|| blank(who, rule));
            row.items += count;
            row.quantity += qty;
            let slot = self
                .quality
                .get(item)
                .map_or(6, |q| usize::from((*q).min(5)));
            row.quality[slot] += count;
        }
        for ((who, rule), t) in &totals {
            let row = rows
                .entry((who.clone(), rule.clone()))
                .or_insert_with(|| blank(who, rule));
            row.rolls_won += t.won;
            row.rolls_lost += t.lost;
            row.rolls_passed += t.passed;
        }
        rows.into_values().collect()
    }
}

pub const DROPS_HEADER: &str =
    "npc,npc_name,item,item_name,rule,threshold,group_size,kills,looted,drops,quantity,rate\n";

pub fn drops_csv(rows: &[DropRow]) -> String {
    let mut s = String::from(DROPS_HEADER);
    for r in rows {
        s.push_str(&csv(&[
            r.npc.clone(),
            r.npc_name.clone(),
            r.item.clone(),
            r.item_name.clone(),
            r.rule.method.clone(),
            r.rule.threshold.clone(),
            r.rule.size.to_string(),
            r.kills.to_string(),
            r.looted.to_string(),
            r.drops.to_string(),
            r.quantity.to_string(),
            r.rate().map(|v| format!("{v:.4}")).unwrap_or_default(),
        ]));
    }
    s
}

pub const ATTRIBUTION_HEADER: &str = "character,rule,threshold,group_size,items,quantity,poor,common,uncommon,rare,epic,legendary,unknown,rolls_won,rolls_lost,rolls_passed\n";

pub fn attribution_csv(rows: &[AttrRow]) -> String {
    let mut s = String::from(ATTRIBUTION_HEADER);
    for r in rows {
        let mut f = vec![
            r.who.clone(),
            r.rule.method.clone(),
            r.rule.threshold.clone(),
            r.rule.size.to_string(),
            r.items.to_string(),
            r.quantity.to_string(),
        ];
        f.extend(r.quality.iter().map(u64::to_string));
        f.push(r.rolls_won.to_string());
        f.push(r.rolls_lost.to_string());
        f.push(r.rolls_passed.to_string());
        s.push_str(&csv(&f));
    }
    s
}

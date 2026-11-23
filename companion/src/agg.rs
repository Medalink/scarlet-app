//! Aggregates computed by scanning events. Every aggregate comes from the
//! REGISTRY table keyed by code, so a new code is one entry; unknown codes
//! contribute nothing and never fail.

type Handler = fn(&mut Agg, &[String], i64);

#[derive(Default)]
pub struct Agg {
    pub kills: u64,
    pub casts: u64,
    pub deaths: u64,
    pub quests: u64,
    pub loot: u64,
    pub camps: u64,
    pub buffs: u64,
    pub xp: i64,
    pub earned: i64,
    pub spent: i64,
    pub jumps: u64,
    /// Yards by movement mode (W lines).
    pub foot_yd: i64,
    pub walk_yd: i64,
    pub mount_yd: i64,
    pub swim_yd: i64,
    pub fly_yd: i64,
    /// Seconds by movement mode.
    pub foot_s: i64,
    pub walk_s: i64,
    pub mount_s: i64,
    pub swim_s: i64,
    pub fly_s: i64,
    pub combat_s: i64,
    pub afk_s: i64,
    pub camp_s: i64,
    pub taxi_s: i64,
    pub rested_s: i64,
    pub group_s: i64,
    pub cast_s: i64,
    pub cc_s: i64,
    /// Damage meter totals (DM lines) and the best damage per second in one fight.
    pub damage: i64,
    pub healing: i64,
    pub taken: i64,
    pub best_dps: i64,
    pub fights: u64,
    pub completed: u64,
    pub abandoned: u64,
    pub emotes: u64,
    pub lols: u64,
    pub rips: u64,
    pub discoveries: u64,
    pub crafted: i64,
    pub duels_won: u64,
    pub duels_lost: u64,
    /// CT counts of your own chat (messages and words sent), IV inspections.
    pub chat_sent: u64,
    pub chat_words: u64,
    pub inspections: u64,
    pub repaired: i64,
    pub screenshots: u64,
    /// CV end lines: cinematics and movies that ran to the end, and that you skipped.
    pub cinematics_watched: u64,
    pub cinematics_skipped: u64,
    pub npc_talks: u64,
    pub levels: u64,
    pub collected: u64,
    pub honor_kills: i64,
    /// CB lines: crafts started; first crafts and concentration from CR.
    pub crafts_started: u64,
    pub first_crafts: u64,
    pub concentration: i64,
    /// LO loot windows by source kind (`c` creature, `g` object, `i` bag item, `o` other).
    pub loot_sources: std::collections::BTreeMap<String, u64>,
    pub mates: std::collections::HashSet<String>,
    /// Distinct creatures seen (NS), players met (PM), vendors and trainers catalogued (VS, TC), items
    /// identified (II).
    pub creatures: std::collections::HashSet<String>,
    pub players_met: std::collections::HashSet<String>,
    pub vendors: std::collections::HashSet<String>,
    pub trainers: std::collections::HashSet<String>,
    pub items_known: std::collections::HashSet<String>,
    /// Open spans per character, so "All characters" never closes one
    /// character's span with another's line.
    open: std::collections::HashMap<u32, Spans>,
    gates: std::collections::HashMap<u32, crate::events::Deaths>,
    ch: u32,
}

#[derive(Default)]
struct Spans {
    afk: Option<i64>,
    rested: Option<i64>,
    group: Option<i64>,
}

impl Agg {
    pub fn new() -> Self {
        Agg::default()
    }

    pub fn apply(&mut self, ch: u32, code: &str, fields: &[String], ms: i64) {
        self.ch = ch;
        if let Some((_, h)) = REGISTRY.iter().find(|(c, _)| *c == code) {
            h(self, fields, ms);
        }
    }

    /// Steps: strides, the addon's own estimate (running 2.5 yd, walking 1.4 yd).
    pub fn steps(&self) -> u64 {
        (self.foot_yd as f64 / 2.5 + self.walk_yd as f64 / 1.4) as u64
    }

    pub fn distance(&self) -> i64 {
        self.foot_yd + self.walk_yd + self.mount_yd + self.swim_yd + self.fly_yd
    }

    /// Seconds of every moving mode combined.
    pub fn moving_s(&self) -> i64 {
        self.foot_s + self.walk_s + self.mount_s + self.swim_s + self.fly_s
    }

    fn close_spans(&mut self, ms: i64) {
        let o = self.open.entry(self.ch).or_default();
        self.afk_s += span(&mut o.afk, ms);
        self.rested_s += span(&mut o.rested, ms);
        self.group_s += span(&mut o.group, ms);
    }

    fn spans(&mut self) -> &mut Spans {
        self.open.entry(self.ch).or_default()
    }
}

fn span(start: &mut Option<i64>, ms: i64) -> i64 {
    start.take().map_or(0, |s| (ms - s).max(0) / 1000)
}

fn toggle(start: &mut Option<i64>, on: bool, ms: i64) -> i64 {
    if on {
        start.get_or_insert(ms);
        0
    } else {
        span(start, ms)
    }
}

fn g(f: &[String], i: usize) -> &str {
    f.get(i).map(String::as_str).unwrap_or("")
}

fn num(f: &[String], i: usize) -> i64 {
    g(f, i).parse().unwrap_or(0)
}

const REGISTRY: &[(&str, Handler)] = &[
    ("K", |a, _f, _ms| a.kills += 1),
    ("C", |a, f, _ms| {
        a.casts += 1;
        a.cast_s += num(f, 1);
    }),
    ("CH", |a, f, _ms| a.cast_s += num(f, 1)),
    ("D", |a, _f, ms| {
        if a.gates.entry(a.ch).or_default().died(ms) {
            a.deaths += 1;
        }
    }),
    ("Q", |a, f, _ms| match g(f, 0) {
        "1" => a.quests += 1,
        "2" => a.completed += 1,
        "3" => a.abandoned += 1,
        _ => {}
    }),
    ("S", |a, _f, ms| a.close_spans(ms)),
    ("ES", |a, _f, ms| a.close_spans(ms)),
    ("RS", |a, f, ms| {
        let s = toggle(&mut a.spans().rested, g(f, 0) == "1", ms);
        a.rested_s += s;
    }),
    ("GP", |a, f, ms| {
        let s = toggle(&mut a.spans().group, g(f, 0) == "1", ms);
        a.group_s += s;
    }),
    ("GM", |a, f, _ms| {
        if g(f, 0) == "1" && !g(f, 1).is_empty() {
            a.mates.insert(g(f, 1).to_string());
        }
    }),
    ("DM", |a, f, _ms| match g(f, 0) {
        "d" => {
            a.damage += num(f, 1);
            a.fights += 1;
            a.best_dps = a.best_dps.max(num(f, 2));
        }
        "h" => a.healing += num(f, 1),
        "t" => a.taken += num(f, 1),
        _ => {}
    }),
    ("EM", |a, _f, _ms| a.emotes += 1),
    ("WD", |a, f, _ms| match g(f, 0) {
        "lol" => a.lols += num(f, 1).max(1) as u64,
        "rip" => a.rips += num(f, 1).max(1) as u64,
        _ => {}
    }),
    ("CT", |a, f, _ms| {
        a.chat_sent += num(f, 2).max(0) as u64;
        a.chat_words += num(f, 3).max(0) as u64;
    }),
    ("IV", |a, _f, _ms| a.inspections += 1),
    ("NS", |a, f, _ms| {
        a.creatures.insert(g(f, 0).to_string());
    }),
    ("PM", |a, f, _ms| {
        a.players_met.insert(g(f, 0).to_string());
    }),
    ("VS", |a, f, _ms| {
        a.vendors.insert(g(f, 0).to_string());
    }),
    ("TC", |a, f, _ms| {
        a.trainers.insert(g(f, 0).to_string());
    }),
    ("II", |a, f, _ms| {
        a.items_known.insert(g(f, 0).to_string());
    }),
    ("EX", |a, _f, _ms| a.discoveries += 1),
    ("CR", |a, f, _ms| {
        a.crafted += num(f, 1).max(1);
        a.concentration += num(f, 7);
        if g(f, 9) == "1" {
            a.first_crafts += 1;
        }
    }),
    ("CB", |a, _f, _ms| a.crafts_started += 1),
    ("LO", |a, f, _ms| {
        if !g(f, 0).is_empty() {
            *a.loot_sources.entry(g(f, 0).to_string()).or_default() += 1;
        }
    }),
    ("DW", |a, f, _ms| {
        if g(f, 0) == "1" {
            a.duels_won += 1;
        } else {
            a.duels_lost += 1;
        }
    }),
    ("RE", |a, f, _ms| a.repaired += num(f, 0)),
    ("SS", |a, _f, _ms| a.screenshots += 1),
    ("CV", |a, f, _ms| {
        if g(f, 0) == "0" {
            match g(f, 4) {
                "c" => a.cinematics_skipped += 1,
                "w" => a.cinematics_watched += 1,
                _ => {}
            }
        }
    }),
    ("NI", |a, _f, _ms| a.npc_talks += 1),
    ("L", |a, _f, _ms| a.levels += 1),
    ("NC", |a, _f, _ms| a.collected += 1),
    ("HK", |a, f, _ms| a.honor_kills += num(f, 1)),
    ("CC", |a, f, _ms| a.cc_s += num(f, 2)),
    ("O", |a, _f, _ms| a.loot += 1),
    ("CP", |a, f, _ms| {
        if g(f, 0) == "1" {
            let reload_at_same_camp = g(f, 2) == "1";
            if !reload_at_same_camp {
                a.camps += 1;
            }
        } else {
            a.camp_s += num(f, 1);
        }
    }),
    ("A", |a, f, _ms| {
        let gained = g(f, 0) == "1";
        let helpful = g(f, 2) == "1";
        if gained && helpful {
            a.buffs += 1;
        }
    }),
    ("X", |a, f, _ms| a.xp += num(f, 0)),
    ("M", |a, f, _ms| {
        let mailed_between_own_characters = num(f, 1);
        let c = num(f, 0) - mailed_between_own_characters;
        if c > 0 {
            a.earned += c;
        } else {
            a.spent += -c;
        }
    }),
    ("W", |a, f, _ms| {
        let (yd, s) = (num(f, 1), num(f, 2));
        a.jumps += num(f, 3) as u64;
        match g(f, 0) {
            "f" => {
                a.foot_yd += yd;
                a.foot_s += s;
            }
            "w" => {
                a.walk_yd += yd;
                a.walk_s += s;
            }
            "m" => {
                a.mount_yd += yd;
                a.mount_s += s;
            }
            "s" => {
                a.swim_yd += yd;
                a.swim_s += s;
            }
            "a" => {
                a.fly_yd += yd;
                a.fly_s += s;
            }
            _ => {}
        }
    }),
    ("F", |a, f, _ms| {
        if g(f, 0) == "0" {
            a.combat_s += num(f, 1);
        }
    }),
    ("T", |a, f, _ms| {
        if g(f, 0) == "0" {
            a.taxi_s += num(f, 4);
        }
    }),
    ("AFK", |a, f, ms| {
        let s = toggle(&mut a.spans().afk, g(f, 0) == "1", ms);
        a.afk_s += s;
    }),
];

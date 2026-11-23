//! Feats counted from what the recording holds, for the character's own screens: the app's Feats view while uploading is
//! off, `Feats.lua`, and the feats of a session recap. A feat is counted here only when the recording can hold its
//! count (`computable`); the rest are counted on the website.

use crate::feats_catalog::{CatalogFeat, FEATS, ROLE_CLASSES, SPEC_ROLES};
use crate::recap::{FeatChange, FeatNear, Feats, SessionRecap, Tally};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const COUNTER_KEYS: [&str; 46] = [
    "kills",
    "casts",
    "deaths",
    "foot",
    "walk",
    "mount",
    "swim",
    "fly",
    "jumps",
    "xp",
    "earned",
    "spent",
    "loot",
    "quests",
    "dungeons",
    "bosses",
    "combat",
    "flights",
    "levels",
    "achievements",
    "camps",
    "camptime",
    "buffs",
    "footTime",
    "walkTime",
    "mountTime",
    "swimTime",
    "flyTime",
    "taxiTime",
    "castTime",
    "afkTime",
    "restedTime",
    "groupTime",
    "emotes",
    "saidLol",
    "saidRip",
    "damage",
    "healing",
    "damageTaken",
    "discoveries",
    "repairs",
    "npcTalks",
    "abandoned",
    "crafted",
    "honorKills",
    "ccTaken",
];
const MIN_SPEC_LEVEL: u32 = 10;
const NEAR: usize = 3;
const SCORE_SCOPE: &str = "general";
pub const TIER_COUNT: usize = 5;

fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn same(a: &str, b: &str) -> bool {
    norm(a) == norm(b)
}

/// Whether the recording can hold this feat's count.
pub fn computable(feat: &CatalogFeat) -> bool {
    let Some(live) = &feat.live else { return false };
    match live.kind {
        "counter" | "session" => live.keys.iter().all(|k| COUNTER_KEYS.contains(k)),
        "level" | "clock" => true,
        "cast" => !feat.spells.is_empty(),
        _ => false,
    }
}

/// Whether a character of `class` can hold this feat at all.
pub fn can_hold(feat: &CatalogFeat, class: &str) -> bool {
    if let Some(c) = feat.class
        && !same(c, class)
    {
        return false;
    }
    if let Some(role) = feat.role {
        return ROLE_CLASSES
            .iter()
            .find(|(r, _)| *r == role)
            .is_some_and(|(_, classes)| classes.iter().any(|c| same(c, class)));
    }
    true
}

fn roles_of(class: &str, tree: &str) -> &'static [&'static str] {
    SPEC_ROLES
        .iter()
        .find(|(c, _)| same(c, class))
        .and_then(|(_, trees)| trees.iter().find(|(t, _)| same(t, tree)))
        .map_or(&[], |(_, roles)| *roles)
}

fn gate_holds(feat: &CatalogFeat, gate: Option<&str>, class: &str, t: &Tally) -> bool {
    match gate {
        None => true,
        Some("class") => feat.class.is_none_or(|c| same(c, class)),
        Some("spec") => feat.tree.is_some_and(|tree| same(tree, &t.spec)),
        Some("role") => feat
            .role
            .is_some_and(|role| roles_of(class, &t.spec).contains(&role)),
        Some(_) => false,
    }
}

fn sum(t: &Tally, keys: &[&str]) -> i64 {
    keys.iter()
        .map(|k| t.counters.get(k).copied().unwrap_or(0))
        .sum()
}

fn value(feat: &CatalogFeat, class: &str, tallies: &[Tally]) -> f64 {
    let live = feat.live.as_ref().expect("computable feats are live");
    let per = live.per.unwrap_or(1.0);
    match live.kind {
        "counter" => tallies.iter().map(|t| sum(t, live.keys)).sum::<i64>() as f64 / per,
        "session" => tallies.iter().map(|t| sum(t, live.keys)).max().unwrap_or(0) as f64 / per,
        "level" if feat.key == "unbroken" => unbroken(tallies),
        "level" => tallies.iter().map(|t| t.level[1]).max().unwrap_or(0) as f64,
        "clock" => {
            let held = tallies
                .iter()
                .filter(|t| gate_holds(feat, live.gate, class, t));
            match live.once_at {
                Some(at) => held.filter(|t| t.played as f64 >= at).count() as f64,
                None => held.map(|t| t.played).sum::<i64>() as f64 / per,
            }
        }
        "cast" => tallies
            .iter()
            .filter(|t| gate_holds(feat, live.gate, class, t))
            .map(|t| {
                feat.spells
                    .iter()
                    .map(|s| t.casts.get(*s).copied().unwrap_or(0))
                    .sum::<i64>()
            })
            .sum::<i64>() as f64,
        _ => 0.0,
    }
}

fn unbroken(tallies: &[Tally]) -> f64 {
    let mut best = 0;
    for t in tallies {
        if t.counters.get("deaths").copied().unwrap_or(0) > 0 {
            return best.max(t.level[0]) as f64;
        }
        best = best.max(t.level[1]);
    }
    best as f64
}

/// One counted feat: its value in the units of its thresholds and the tier that value reaches.
pub struct Counted {
    pub feat: &'static CatalogFeat,
    pub v: f64,
    pub tier: u8,
}

impl Counted {
    pub fn next(&self) -> Option<f64> {
        self.feat.thresholds.get(self.tier as usize).copied()
    }

    pub fn left(&self) -> Option<f64> {
        self.next().map(|n| (n - self.v).max(0.0))
    }

    /// How far toward the next tier, from the tier's own threshold, 0 to 1; 1 at the top tier.
    pub fn progress(&self) -> f64 {
        let Some(next) = self.next() else { return 1.0 };
        let from = match self.tier {
            0 => 0.0,
            t => self.feat.thresholds[t as usize - 1],
        };
        ((self.v - from) / (next - from)).clamp(0.0, 1.0)
    }
}

pub fn tier_of(feat: &CatalogFeat, v: f64) -> u8 {
    feat.thresholds.iter().filter(|t| v >= **t).count() as u8
}

pub struct Scored {
    pub class: String,
    pub through: i64,
    pub spec: String,
    pub can_hold: usize,
    pub counted: Vec<Counted>,
}

impl Scored {
    pub fn held(&self) -> usize {
        self.counted.iter().filter(|c| c.tier > 0).count()
    }

    pub fn total(&self) -> usize {
        self.counted.len()
    }

    /// Feats of this class the recording cannot count.
    pub fn elsewhere(&self) -> usize {
        self.can_hold - self.counted.len()
    }

    /// One point for each tier held on the general feats that are not gold feats.
    pub fn score(&self) -> u32 {
        self.counted
            .iter()
            .filter(|c| c.feat.scope == SCORE_SCOPE && !c.feat.economy)
            .map(|c| c.tier as u32)
            .sum()
    }

    pub fn score_max(&self) -> u32 {
        FEATS
            .iter()
            .filter(|f| f.scope == SCORE_SCOPE && !f.economy)
            .count() as u32
            * TIER_COUNT as u32
    }

    pub fn tier_counts(&self) -> [usize; TIER_COUNT] {
        let mut out = [0; TIER_COUNT];
        for c in self.counted.iter().filter(|c| c.tier > 0) {
            out[c.tier as usize - 1] += 1;
        }
        out
    }

    /// The feats with a count and a tier still to reach, those nearest it first.
    pub fn closest(&self, n: usize) -> Vec<&Counted> {
        let mut rows: Vec<&Counted> = self
            .counted
            .iter()
            .filter(|c| c.v > 0.0 && c.next().is_some())
            .collect();
        rows.sort_by(|a, b| {
            b.progress()
                .total_cmp(&a.progress())
                .then_with(|| a.feat.key.cmp(b.feat.key))
        });
        rows.truncate(n);
        rows
    }
}

fn identity(tallies: &[Tally]) -> (String, String) {
    let class = tallies
        .iter()
        .rev()
        .find(|t| !t.class.is_empty())
        .map_or_else(String::new, |t| t.class.clone());
    let spec = tallies
        .iter()
        .rev()
        .find(|t| !t.spec.is_empty() && t.level[1] >= MIN_SPEC_LEVEL)
        .map_or_else(String::new, |t| t.spec.clone());
    (class, spec)
}

/// Count the feats of a character from its sessions in time order.
pub fn score(tallies: &[Tally]) -> Scored {
    let (class, spec) = identity(tallies);
    let counted = FEATS
        .iter()
        .filter(|f| computable(f) && can_hold(f, &class))
        .map(|f| {
            let v = value(f, &class, tallies);
            Counted {
                feat: f,
                v,
                tier: tier_of(f, v),
            }
        })
        .collect();
    Scored {
        can_hold: FEATS.iter().filter(|f| can_hold(f, &class)).count(),
        class,
        through: tallies.last().map_or(0, |t| t.start),
        spec,
        counted,
    }
}

/// The sum of a counter feat's keys in the lifetime counters the addon keeps, for `b` in Feats.lua.
pub fn lifetime_base(feat: &CatalogFeat, lifetime: &BTreeMap<String, i64>) -> Option<i64> {
    let live = feat.live.as_ref()?;
    (live.kind == "counter").then(|| {
        live.keys
            .iter()
            .map(|k| lifetime.get(*k).copied().unwrap_or(0))
            .sum()
    })
}

/// Fills `recap.feats` from the sessions before the recap and the ones it holds.
pub fn fill(recap: &mut SessionRecap, tallies: &[Tally]) {
    let first = recap.start;
    let last = recap.end;
    let before: Vec<Tally> = tallies
        .iter()
        .filter(|t| t.start < first)
        .cloned()
        .collect();
    let upto: Vec<Tally> = tallies
        .iter()
        .filter(|t| t.start <= last)
        .cloned()
        .collect();
    if upto.is_empty() {
        return;
    }
    let (was, now) = (score(&before), score(&upto));
    let at = last - first;
    let earned = now
        .counted
        .iter()
        .filter_map(|c| {
            let from = was
                .counted
                .iter()
                .find(|w| w.feat.key == c.feat.key)
                .map_or(0, |w| w.tier);
            (c.tier > from).then(|| FeatChange {
                key: c.feat.key.to_string(),
                from,
                tier: c.tier,
                at,
                value: c.v,
            })
        })
        .collect();
    let near = now
        .closest(NEAR)
        .into_iter()
        .filter_map(|c| {
            Some(FeatNear {
                key: c.feat.key.to_string(),
                tier: c.tier,
                value: c.v,
                next: c.next()?,
                left: c.left()?,
            })
        })
        .collect();
    recap.feats = Feats { earned, near };
}

/// What a character's counts come to, kept beside its overview so Feats.lua can be written without its events.
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Saved {
    pub through: i64,
    pub spec: String,
    pub values: BTreeMap<String, f64>,
}

pub fn saved(scored: &Scored) -> Saved {
    Saved {
        through: scored.through,
        spec: scored.spec.clone(),
        values: scored
            .counted
            .iter()
            .map(|c| (c.feat.key.to_string(), c.v))
            .collect(),
    }
}

/// What the addon's own file says about one character: its lifetime counters and the tier of each feat it has toasted.
#[derive(Default, Clone)]
pub struct Kept {
    pub lifetime: BTreeMap<String, i64>,
    pub seen: Option<BTreeMap<String, i64>>,
}

fn lua_num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{:.3}", v)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

/// The text of Feats.lua, `source = "local"`. A counter feat is written only for a character whose lifetime counters
/// are in the addon's file, since `b` is read from them; `s` is the tier the addon has toasted, or the current tier
/// for a character whose addon has toasted nothing yet.
pub fn feats_lua(chars: &[(String, Saved, Kept)], made: i64) -> String {
    let mut out = format!(
        "ScarletFeats = {{\n  v = 1, made = {made},\n  catalog = {},\n  source = \"local\",\n  chars = {{\n",
        crate::feats_catalog::VERSION
    );
    for (name, saved, kept) in chars {
        let rows: Vec<(&CatalogFeat, f64, u8, Option<i64>)> = FEATS
            .iter()
            .filter_map(|f| {
                let v = *saved.values.get(f.key)?;
                let b = lifetime_base(f, &kept.lifetime);
                let is_counter = f.live.as_ref().is_some_and(|l| l.kind == "counter");
                if is_counter && (kept.lifetime.is_empty() || b.is_none()) {
                    return None;
                }
                Some((f, v, tier_of(f, v), b))
            })
            .collect();
        let held = rows.iter().filter(|r| r.2 > 0).count();
        let score: u32 = rows
            .iter()
            .filter(|r| r.0.scope == SCORE_SCOPE && !r.0.economy)
            .map(|r| r.2 as u32)
            .sum();
        out.push_str(&format!("    [{}] = {{\n", lua_quote(name)));
        out.push_str(&format!("      through = {},\n", saved.through));
        if !saved.spec.is_empty() {
            out.push_str(&format!("      spec = {},\n", lua_quote(&saved.spec)));
        }
        out.push_str(&format!(
            "      held = {held}, total = {}, score = {score},\n      pins = {{}},\n      f = {{\n",
            rows.len()
        ));
        let seen_of = |f: &CatalogFeat, tier: u8| -> u8 {
            match &kept.seen {
                Some(seen) => seen.get(f.key).copied().unwrap_or(0).clamp(0, 5) as u8,
                None => tier,
            }
        };
        for (f, v, tier, b) in &rows {
            let s = seen_of(f, *tier);
            let base = b.map_or(String::new(), |b| format!(", b = {b}"));
            out.push_str(&format!(
                "        [{}] = {{ v = {}, s = {s}{base} }},\n",
                lua_quote(f.key),
                lua_num(*v)
            ));
        }
        out.push_str("      },\n      new = {");
        let news: Vec<String> = rows
            .iter()
            .filter(|(f, _, tier, _)| *tier > seen_of(f, *tier))
            .map(|(f, _, tier, _)| {
                format!(
                    "{{ k = {}, from = {}, to = {tier} }}",
                    lua_quote(f.key),
                    seen_of(f, *tier)
                )
            })
            .collect();
        out.push_str(&news.join(", "));
        out.push_str("},\n    },\n");
    }
    out.push_str("  },\n}\n");
    out
}

fn lua_quote(s: &str) -> String {
    let mut out = String::new();
    crate::overview::lua_write(&serde_json::Value::String(s.to_string()), 1, &mut out);
    out
}

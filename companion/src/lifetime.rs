//! What the addon's lifetime lines say about one character: quests completed (`QB`), titles known
//! (`KT`), zone maps explored (`EB`), instance locks (`IS`), auction prices (`AQ`) and recipes
//! (`RQ`), each as the newest read (docs/FORMAT.md). Feeds the app's Overview and Items views.

use crate::events::difficulty_name;
use crate::overview::Ev;
use serde::Serialize;
use std::collections::BTreeMap;

pub const CODES: [&str; 6] = ["QB", "KT", "EB", "IS", "AQ", "RQ"];

#[derive(Serialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct Counted {
    pub total: i64,
    pub at: i64,
}

#[derive(Serialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct Lock {
    pub name: String,
    pub kind: &'static str,
    pub difficulty: String,
    pub done: Option<i64>,
    pub total: Option<i64>,
    pub players: Option<i64>,
    pub locked: bool,
    pub extended: bool,
    pub reset_at: i64,
}

#[derive(Serialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct Price {
    pub item: i64,
    pub kind: &'static str,
    pub copper: i64,
    pub quantity: i64,
    pub listings: i64,
    pub item_level: String,
    pub at: i64,
}

#[derive(Serialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct Reagent {
    pub item: i64,
    pub count: i64,
}

#[derive(Serialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct Recipe {
    pub spell: i64,
    pub item: i64,
    pub min: i64,
    pub max: i64,
    pub reagents: Vec<Reagent>,
    pub optional: i64,
    pub at: i64,
}

#[derive(Serialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct Lifetime {
    pub quests: Option<Counted>,
    pub titles: Option<Counted>,
    pub explored: Option<Counted>,
    pub locks: Vec<Lock>,
    pub prices: Vec<Price>,
    pub recipes: Vec<Recipe>,
}

impl Lifetime {
    pub fn is_empty(&self) -> bool {
        self.quests.is_none()
            && self.titles.is_none()
            && self.explored.is_none()
            && self.locks.is_empty()
            && self.prices.is_empty()
            && self.recipes.is_empty()
    }

    /// Drops the locks that have reset by `now` (Unix seconds), soonest reset first.
    pub fn keep_current_locks(&mut self, now: i64) {
        self.locks
            .retain(|l| l.reset_at > now && (l.locked || l.extended));
        self.locks.sort_by_key(|l| l.reset_at);
    }
}

fn number(s: &str) -> Option<i64> {
    s.trim().parse().ok()
}

fn price_kind(v: &str) -> Option<&'static str> {
    match v {
        "c" => Some("commodity"),
        "i" => Some("item"),
        "b" => Some("browse"),
        _ => None,
    }
}

pub fn build<'a>(events: impl IntoIterator<Item = Ev<'a>>) -> Lifetime {
    let mut out = Lifetime::default();
    let mut maps: BTreeMap<i64, i64> = BTreeMap::new();
    let mut maps_at = 0;
    let mut last = "";
    let mut locks: BTreeMap<(&'static str, String, String), Lock> = BTreeMap::new();
    let mut prices: BTreeMap<i64, Price> = BTreeMap::new();
    let mut recipes: BTreeMap<i64, Recipe> = BTreeMap::new();
    for (ms, code, f) in events {
        let previous = std::mem::replace(&mut last, code);
        if !CODES.contains(&code) {
            continue;
        }
        let field = |i: usize| f.get(i).map_or("", String::as_str);
        let at = ms.div_euclid(1000);
        match code {
            "QB" | "KT" => {
                let Some(total) = number(field(0)) else {
                    continue;
                };
                let counted = Some(Counted { total, at });
                if code == "QB" {
                    out.quests = counted;
                } else {
                    out.titles = counted;
                }
            }
            "EB" => {
                if previous != "EB" {
                    maps.clear();
                    maps_at = at;
                }
                for part in field(0).split(',') {
                    if let Some((map, n)) = part.split_once(':')
                        && let (Some(map), Some(n)) = (number(map), number(n))
                        && n > 0
                    {
                        maps.insert(map, n);
                    }
                }
                out.explored = Some(Counted {
                    total: maps.len() as i64,
                    at: maps_at,
                });
            }
            "IS" => {
                let Some(seconds) = number(field(3)) else {
                    continue;
                };
                let kind = match field(6) {
                    "2" => "world boss",
                    "1" => "raid",
                    _ => "dungeon",
                };
                let which = if field(1).is_empty() {
                    field(0)
                } else {
                    field(1)
                };
                let key = (kind, which.to_string(), field(2).to_string());
                let name = if field(0).is_empty() {
                    "Instance"
                } else {
                    field(0)
                };
                let difficulty = if kind == "world boss" || field(2).is_empty() {
                    String::new()
                } else {
                    difficulty_name(field(2))
                };
                locks.insert(
                    key,
                    Lock {
                        name: name.to_string(),
                        kind,
                        difficulty,
                        done: number(field(8)),
                        total: number(field(9)),
                        players: number(field(7)),
                        locked: field(4) == "1",
                        extended: field(5) == "1",
                        reset_at: at + seconds,
                    },
                );
            }
            "AQ" => {
                let (Some(kind), Some(item), Some(copper)) =
                    (price_kind(field(0)), number(field(1)), number(field(2)))
                else {
                    continue;
                };
                prices.insert(
                    item,
                    Price {
                        item,
                        kind,
                        copper,
                        quantity: number(field(3)).unwrap_or(0),
                        listings: number(field(4)).unwrap_or(0),
                        item_level: field(5).to_string(),
                        at,
                    },
                );
            }
            "RQ" => {
                let Some(spell) = number(field(0)) else {
                    continue;
                };
                let reagents = field(4)
                    .split(',')
                    .filter_map(|part| {
                        let (item, count) = part.split_once(':')?;
                        Some(Reagent {
                            item: number(item)?,
                            count: number(count)?,
                        })
                    })
                    .collect();
                recipes.insert(
                    spell,
                    Recipe {
                        spell,
                        item: number(field(1)).unwrap_or(0),
                        min: number(field(2)).unwrap_or(0),
                        max: number(field(3)).unwrap_or(0),
                        reagents,
                        optional: number(field(5)).unwrap_or(0),
                        at,
                    },
                );
            }
            _ => {}
        }
    }
    out.locks = locks.into_values().collect();
    out.prices = prices.into_values().collect();
    out.recipes = recipes.into_values().collect();
    out
}

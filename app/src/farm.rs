//! What the characters farm and how far each is from its ceilings, across the account: the best record of each kind over
//! all characters, each character's skills and reputation against their caps, the items farmed most with their pace and
//! record, and the first time each rare item was looted. Built from the per-character goals, which are cached.

use crate::commands::{blocking, with_store};
use crate::recap::goals;
use crate::store::Store;
use scarlet::goals::{CapSummary, Farmed, Latest, Record};
use serde::Serialize;
use std::collections::HashMap;
use tauri::AppHandle;

const FARMED_SHOWN: usize = 8;
const FARMED_LOOKED_AT: usize = 40;
const FIRSTS_SHOWN: usize = 6;

#[derive(Serialize)]
pub struct Best {
    character: String,
    #[serde(flatten)]
    record: Record,
}

#[derive(Serialize)]
pub struct CharCaps {
    name: String,
    #[serde(flatten)]
    caps: CapSummary,
}

#[derive(Serialize)]
pub struct AccountLatest {
    character: String,
    #[serde(flatten)]
    latest: Latest,
}

#[derive(Serialize)]
pub struct AccountGoals {
    records: Vec<Best>,
    chars: Vec<CharCaps>,
    latest: Option<AccountLatest>,
}

#[derive(Serialize)]
pub struct Farm {
    character: String,
    #[serde(flatten)]
    item: Farmed,
}

#[derive(Serialize, Default)]
pub struct Farming {
    pub farmed: Vec<Farm>,
    pub firsts: Vec<Farm>,
}

fn scope(st: &Store, only: Option<u32>) -> Vec<u32> {
    match only {
        Some(c) => vec![c],
        None => (0..st.chars.len() as u32).collect(),
    }
}

/// The items looted most while farming them, each drawn from the character who looted most of it in the runs that
/// farmed it, with that character's own total, and the rare items by when they were first looted.
pub fn farming(st: &Store, chars: &[u32]) -> Farming {
    let all: Vec<(u32, std::sync::Arc<scarlet::goals::Goals>)> =
        chars.iter().map(|c| (*c, goals(st, *c))).collect();
    let mut chosen: HashMap<i64, (u32, i64, i64)> = HashMap::new();
    for (c, g) in &all {
        for (id, total) in g.farmed_totals() {
            let Some(count) = g.farming_count(id) else {
                continue;
            };
            let e = chosen.entry(id).or_insert((*c, count, total));
            if (count, total) > (e.1, e.2) {
                *e = (*c, count, total);
            }
        }
    }
    let by = |c: u32| all.iter().find(|(x, _)| *x == c).map(|(_, g)| g);
    let named = |c: u32, item: Farmed| Farm {
        character: st.chars[c as usize].clone(),
        item,
    };
    let mut ranked: Vec<(i64, (u32, i64, i64))> = chosen.into_iter().collect();
    ranked.sort_by(|a, b| b.1.2.cmp(&a.1.2).then(a.0.cmp(&b.0)));
    let mut farmed = Vec::new();
    for (id, (c, _, _)) in ranked.iter().take(FARMED_LOOKED_AT) {
        let Some(item) = by(*c).and_then(|g| g.farmed(*id)) else {
            continue;
        };
        if item.pace.is_none() || item.name.is_empty() {
            continue;
        }
        farmed.push(named(*c, item));
        if farmed.len() == FARMED_SHOWN {
            break;
        }
    }
    let mut firsts: Vec<Farm> = Vec::new();
    for (c, g) in &all {
        for id in g.rare_items() {
            let Some(item) = g.farmed(id).filter(|i| !i.name.is_empty()) else {
                continue;
            };
            match firsts.iter().position(|f| f.item.id == id) {
                Some(at) if firsts[at].item.first[0] <= item.first[0] => {}
                Some(at) => firsts[at] = named(*c, item),
                None => firsts.push(named(*c, item)),
            }
        }
    }
    firsts.sort_by(|a, b| b.item.first[0].cmp(&a.item.first[0]));
    firsts.truncate(FIRSTS_SHOWN);
    Farming { farmed, firsts }
}

#[tauri::command]
pub async fn account_goals(app: AppHandle) -> AccountGoals {
    blocking(move || {
        with_store(&app, |st| {
            let mut records: Vec<Best> = Vec::new();
            let mut chars = Vec::new();
            let mut latest: Option<AccountLatest> = None;
            for c in scope(st, None) {
                let g = goals(st, c);
                let name = st.chars[c as usize].clone();
                for record in g.records() {
                    if record.key() == "balance" {
                        continue;
                    }
                    let held = records.iter().position(|b| b.record.key() == record.key());
                    match held {
                        Some(at) if records[at].record.value() >= record.value() => {}
                        Some(at) => {
                            records[at] = Best {
                                character: name.clone(),
                                record,
                            }
                        }
                        None => records.push(Best {
                            character: name.clone(),
                            record,
                        }),
                    }
                }
                if let Some(l) = g.latest()
                    && latest.as_ref().is_none_or(|n| l.to() > n.latest.to())
                {
                    latest = Some(AccountLatest {
                        character: name.clone(),
                        latest: l,
                    });
                }
                chars.push(CharCaps {
                    name,
                    caps: g.summary(),
                });
            }
            AccountGoals {
                records,
                chars,
                latest,
            }
        })
    })
    .await
}

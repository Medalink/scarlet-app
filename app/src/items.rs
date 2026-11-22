//! Item search across your characters: what each one carries in its bags and bank, from the
//! `inventory.json` the companion writes beside each character's overview.

use crate::commands::with_store;
use scarlet::inventory::Inventory;
use serde::Serialize;
use std::collections::HashMap;
use tauri::AppHandle;

const MAX_ROWS: usize = 300;

#[derive(Serialize)]
pub struct Holder {
    character: String,
    bags: i64,
    bank: i64,
}

#[derive(Serialize)]
pub struct ItemRow {
    id: i64,
    name: String,
    total: i64,
    holders: Vec<Holder>,
}

#[derive(Serialize)]
pub struct Carrier {
    name: String,
    bags_at: i64,
    bank_at: i64,
    stacks: usize,
}

#[derive(Serialize)]
pub struct Items {
    characters: Vec<Carrier>,
    rows: Vec<ItemRow>,
    matched: usize,
}

#[tauri::command(async)]
pub fn items(app: AppHandle, query: String) -> Items {
    let loaded: Vec<(String, Inventory)> = with_store(&app, |st| {
        st.chars
            .iter()
            .filter_map(|name| {
                let text =
                    std::fs::read_to_string(st.data_dir.join(name).join("inventory.json")).ok()?;
                let inv: Inventory = serde_json::from_str(&text).ok()?;
                (!inv.is_empty()).then(|| (name.clone(), inv))
            })
            .collect()
    });
    search(loaded, &query)
}

fn search(loaded: Vec<(String, Inventory)>, query: &str) -> Items {
    let q = query.trim().to_lowercase();
    let mut known: HashMap<i64, &str> = HashMap::new();
    for (_, inv) in &loaded {
        for (id, item_name) in &inv.names {
            if !item_name.is_empty() {
                known.entry(*id).or_insert(item_name);
            }
        }
    }
    let mut by_item: HashMap<i64, ItemRow> = HashMap::new();
    for (name, inv) in &loaded {
        let ids = inv.bags.keys().chain(inv.bank.keys());
        for id in ids {
            let item_name = known.get(id).copied().unwrap_or_default();
            let hit = q.is_empty() || item_name.to_lowercase().contains(&q) || id.to_string() == q;
            if !hit {
                continue;
            }
            let row = by_item.entry(*id).or_insert_with(|| ItemRow {
                id: *id,
                name: item_name.to_string(),
                total: 0,
                holders: Vec::new(),
            });
            if row.holders.iter().any(|h| h.character == *name) {
                continue;
            }
            let (bags, bank) = (
                inv.bags.get(id).copied().unwrap_or(0),
                inv.bank.get(id).copied().unwrap_or(0),
            );
            row.total += bags + bank;
            row.holders.push(Holder {
                character: name.clone(),
                bags,
                bank,
            });
        }
    }
    let mut rows: Vec<ItemRow> = by_item.into_values().collect();
    rows.sort_by(|a, b| {
        b.total
            .cmp(&a.total)
            .then(a.name.cmp(&b.name))
            .then(a.id.cmp(&b.id))
    });
    let matched = rows.len();
    rows.truncate(MAX_ROWS);
    for row in &mut rows {
        row.holders.sort_by(|a, b| {
            (b.bags + b.bank)
                .cmp(&(a.bags + a.bank))
                .then(a.character.cmp(&b.character))
        });
    }
    let characters = loaded
        .iter()
        .map(|(name, inv)| Carrier {
            name: name.clone(),
            bags_at: inv.bags_at,
            bank_at: inv.bank_at,
            stacks: inv.bags.len() + inv.bank.len(),
        })
        .collect();
    Items {
        characters,
        rows,
        matched,
    }
}

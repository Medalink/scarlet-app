//! Item search across your characters: what each one carries in its bags and bank, from the
//! `inventory.json` the companion writes beside each character's overview.

use crate::commands::{state, with_store};
use scarlet::inventory::{Inventory, newest_per};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use tauri::AppHandle;

const MAX_ROWS: usize = 300;

#[derive(Serialize)]
pub struct Holder {
    character: String,
    bags: i64,
    bank: i64,
    mail: i64,
    auctions: i64,
}

#[derive(Serialize)]
pub struct Place {
    place: &'static str,
    scope: String,
    count: i64,
}

#[derive(Serialize)]
pub struct ItemRow {
    id: i64,
    name: String,
    total: i64,
    holders: Vec<Holder>,
    places: Vec<Place>,
}

#[derive(Serialize)]
pub struct Carrier {
    name: String,
    bags_at: i64,
    bank_at: i64,
    mail_at: i64,
    auctions_at: i64,
    stacks: usize,
}

#[derive(Serialize)]
pub struct SharedBank {
    place: &'static str,
    scope: String,
    read_by: String,
    at: i64,
    stacks: usize,
    money: Option<i64>,
}

#[derive(Serialize)]
pub struct Items {
    characters: Vec<Carrier>,
    shared: Vec<SharedBank>,
    rows: Vec<ItemRow>,
    matched: usize,
}

struct Shared<'a> {
    place: &'static str,
    scope: String,
    read_by: &'a str,
    at: i64,
    list: &'a BTreeMap<i64, i64>,
    money: Option<i64>,
}

#[tauri::command(async)]
pub fn items(app: AppHandle, query: String) -> Items {
    let folders = state(&app).settings.lock().unwrap().folders();
    let accounts: Vec<(String, Vec<String>)> = folders
        .iter()
        .flat_map(|f| scarlet::export::accounts(f))
        .map(|a| (a.name, a.characters))
        .collect();
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
    search(loaded, &accounts, &query)
}

fn shared_banks<'a>(
    loaded: &'a [(String, Inventory)],
    accounts: &[(String, Vec<String>)],
) -> Vec<Shared<'a>> {
    let account_of = |name: &str| {
        accounts
            .iter()
            .find(|(_, chars)| chars.iter().any(|c| c == name))
            .map_or_else(String::new, |(account, _)| account.clone())
    };
    let warband_lists = newest_per(
        loaded
            .iter()
            .map(|(name, inv)| (account_of(name), inv.warband_at, (name.as_str(), inv))),
    );
    let warband_money = newest_per(
        loaded
            .iter()
            .map(|(name, inv)| (account_of(name), inv.warband_money_at, inv.warband_money)),
    );
    let guild_of = |name: &str, guild: &str| {
        if guild.is_empty() {
            name.to_string()
        } else {
            guild.to_string()
        }
    };
    let guild_lists = newest_per(loaded.iter().map(|(name, inv)| {
        (
            guild_of(name, &inv.guild_name),
            inv.guild_at,
            (name.as_str(), inv),
        )
    }));
    let guild_money = newest_per(loaded.iter().map(|(name, inv)| {
        (
            guild_of(name, &inv.guild_money_name),
            inv.guild_money_at,
            (name.as_str(), inv.guild_money),
        )
    }));
    static NO_ITEMS: BTreeMap<i64, i64> = BTreeMap::new();
    let mut out = Vec::new();
    let scopes: BTreeSet<&String> = warband_lists.keys().chain(warband_money.keys()).collect();
    for scope in scopes {
        let (list_at, read_by, list) = match warband_lists.get(scope) {
            Some((at, (name, inv))) => (*at, *name, &inv.warband),
            None => (0, "", &NO_ITEMS),
        };
        let money = warband_money.get(scope);
        out.push(Shared {
            place: "Warband bank",
            scope: scope.clone(),
            read_by,
            at: list_at.max(money.map_or(0, |(at, _)| *at)),
            list,
            money: money.map(|(_, copper)| *copper),
        });
    }
    let scopes: BTreeSet<&String> = guild_lists.keys().chain(guild_money.keys()).collect();
    for scope in scopes {
        let (list_at, read_by, list) = match guild_lists.get(scope) {
            Some((at, (name, inv))) => (*at, *name, &inv.guild),
            None => (0, "", &NO_ITEMS),
        };
        let money = guild_money.get(scope);
        out.push(Shared {
            place: "Guild bank",
            scope: scope.clone(),
            read_by,
            at: list_at.max(money.map_or(0, |(at, _)| *at)),
            list,
            money: money.map(|(_, (_, copper))| *copper),
        });
    }
    out
}

fn search(
    loaded: Vec<(String, Inventory)>,
    accounts: &[(String, Vec<String>)],
    query: &str,
) -> Items {
    let q = query.trim().to_lowercase();
    let mut known: HashMap<i64, &str> = HashMap::new();
    for (_, inv) in &loaded {
        for (id, item_name) in &inv.names {
            if !item_name.is_empty() {
                known.entry(*id).or_insert(item_name);
            }
        }
    }
    let shared = shared_banks(&loaded, accounts);
    let item_name = |id: &i64| known.get(id).copied().unwrap_or_default().to_string();
    let mut by_item: HashMap<i64, ItemRow> = HashMap::new();
    let row_for = |id: i64, by_item: &mut HashMap<i64, ItemRow>| -> bool {
        if by_item.contains_key(&id) {
            return true;
        }
        let name = item_name(&id);
        let hit = q.is_empty() || name.to_lowercase().contains(&q) || id.to_string() == q;
        if hit {
            by_item.insert(
                id,
                ItemRow {
                    id,
                    name,
                    total: 0,
                    holders: Vec::new(),
                    places: Vec::new(),
                },
            );
        }
        hit
    };
    for (name, inv) in &loaded {
        let ids = inv
            .bags
            .keys()
            .chain(inv.bank.keys())
            .chain(inv.mail.keys())
            .chain(inv.auctions.keys());
        for id in ids {
            if !row_for(*id, &mut by_item) {
                continue;
            }
            let row = by_item.get_mut(id).expect("row exists");
            if row.holders.iter().any(|h| h.character == *name) {
                continue;
            }
            let count = |m: &BTreeMap<i64, i64>| m.get(id).copied().unwrap_or(0);
            let holder = Holder {
                character: name.clone(),
                bags: count(&inv.bags),
                bank: count(&inv.bank),
                mail: count(&inv.mail),
                auctions: count(&inv.auctions),
            };
            row.total += holder.bags + holder.bank + holder.mail + holder.auctions;
            row.holders.push(holder);
        }
    }
    for bank in &shared {
        for (id, count) in bank.list {
            if !row_for(*id, &mut by_item) {
                continue;
            }
            let row = by_item.get_mut(id).expect("row exists");
            row.total += count;
            row.places.push(Place {
                place: bank.place,
                scope: bank.scope.clone(),
                count: *count,
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
            let held = |h: &Holder| h.bags + h.bank + h.mail + h.auctions;
            held(b).cmp(&held(a)).then(a.character.cmp(&b.character))
        });
    }
    let characters = loaded
        .iter()
        .map(|(name, inv)| Carrier {
            name: name.clone(),
            bags_at: inv.bags_at,
            bank_at: inv.bank_at,
            mail_at: inv.mail_at,
            auctions_at: inv.auctions_at,
            stacks: inv.bags.len() + inv.bank.len() + inv.mail.len() + inv.auctions.len(),
        })
        .collect();
    let shared = shared
        .iter()
        .map(|bank| SharedBank {
            place: bank.place,
            scope: bank.scope.clone(),
            read_by: bank.read_by.to_string(),
            at: bank.at,
            stacks: bank.list.len(),
            money: bank.money,
        })
        .collect();
    Items {
        characters,
        shared,
        rows,
        matched,
    }
}

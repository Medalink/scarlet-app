//! Item search across your characters: what each one carries in its bags and bank, from the
//! `inventory.json` the companion writes beside each character's overview.

use crate::commands::{state, with_store};
use scarlet::inventory::{Inventory, newest_per};
use scarlet::lifetime::{Lifetime, Price};
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
    account_unknown: bool,
    count: i64,
}

#[derive(Serialize)]
pub struct SeenPrice {
    copper: i64,
    kind: &'static str,
    at: i64,
}

#[derive(Serialize)]
pub struct ItemRow {
    id: i64,
    name: String,
    total: i64,
    holders: Vec<Holder>,
    places: Vec<Place>,
    price: Option<SeenPrice>,
}

#[derive(Serialize)]
pub struct ReagentRow {
    id: i64,
    name: String,
    count: i64,
}

#[derive(Serialize)]
pub struct RecipeRow {
    character: String,
    name: String,
    makes: String,
    min: i64,
    max: i64,
    reagents: Vec<ReagentRow>,
}

struct Known {
    items: HashMap<i64, String>,
    spells: HashMap<i64, String>,
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
    account_unknown: bool,
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
    recipes: Vec<RecipeRow>,
    recipes_matched: usize,
}

struct Shared<'a> {
    place: &'static str,
    scope: String,
    account_unknown: bool,
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
    let (loaded, lives, known) = with_store(&app, |st| {
        let loaded: Vec<(String, Inventory)> = st
            .chars
            .iter()
            .filter_map(|name| {
                let text =
                    std::fs::read_to_string(st.data_dir.join(name).join("inventory.json")).ok()?;
                let inv: Inventory = serde_json::from_str(&text).ok()?;
                (!inv.is_empty()).then(|| (name.clone(), inv))
            })
            .collect();
        let lives: Vec<(String, Lifetime)> = st
            .chars
            .iter()
            .cloned()
            .zip(st.lifetimes.iter().cloned())
            .collect();
        let named = |kind: char| -> HashMap<i64, String> {
            st.names
                .iter()
                .filter_map(|(key, name)| {
                    let id = key.strip_prefix(kind)?.parse().ok()?;
                    Some((id, name.clone()))
                })
                .collect()
        };
        (
            loaded,
            lives,
            Known {
                items: named('i'),
                spells: named('s'),
            },
        )
    });
    search(loaded, &lives, &known, &accounts, &query)
}

fn recipe_rows(lives: &[(String, Lifetime)], known: &Known, q: &str) -> Vec<RecipeRow> {
    let item_name = |id: i64| known.items.get(&id).cloned().unwrap_or_default();
    let mut rows: Vec<RecipeRow> = lives
        .iter()
        .flat_map(|(character, life)| life.recipes.iter().map(move |r| (character, r)))
        .map(|(character, r)| RecipeRow {
            character: character.clone(),
            name: known.spells.get(&r.spell).cloned().unwrap_or_default(),
            makes: item_name(r.item),
            min: r.min,
            max: r.max,
            reagents: r
                .reagents
                .iter()
                .map(|g| ReagentRow {
                    id: g.item,
                    name: item_name(g.item),
                    count: g.count,
                })
                .collect(),
        })
        .filter(|r| {
            q.is_empty()
                || r.name.to_lowercase().contains(q)
                || r.makes.to_lowercase().contains(q)
                || r.reagents
                    .iter()
                    .any(|g| g.name.to_lowercase().contains(q) || g.id.to_string() == q)
        })
        .collect();
    rows.sort_by(|a, b| a.name.cmp(&b.name).then(a.character.cmp(&b.character)));
    rows
}

fn shared_banks<'a>(
    loaded: &'a [(String, Inventory)],
    accounts: &[(String, Vec<String>)],
) -> Vec<Shared<'a>> {
    let account_of = |name: &str| {
        accounts
            .iter()
            .find(|(_, chars)| chars.iter().any(|c| c == name))
            .map_or_else(
                || (String::new(), name.to_string()),
                |(account, _)| (account.clone(), String::new()),
            )
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
    let scopes: BTreeSet<&(String, String)> =
        warband_lists.keys().chain(warband_money.keys()).collect();
    for scope in scopes {
        let (list_at, read_by, list) = match warband_lists.get(scope) {
            Some((at, (name, inv))) => (*at, *name, &inv.warband),
            None => (0, "", &NO_ITEMS),
        };
        let money = warband_money.get(scope);
        out.push(Shared {
            place: "Warband bank",
            scope: if scope.1.is_empty() {
                scope.0.clone()
            } else {
                scope.1.clone()
            },
            account_unknown: !scope.1.is_empty(),
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
            account_unknown: false,
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
    lives: &[(String, Lifetime)],
    names: &Known,
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
    for (id, item_name) in &names.items {
        known.entry(*id).or_insert(item_name);
    }
    let mut prices: HashMap<i64, &Price> = HashMap::new();
    for price in lives.iter().flat_map(|(_, life)| &life.prices) {
        if prices.get(&price.item).is_none_or(|p| price.at > p.at) {
            prices.insert(price.item, price);
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
                    price: prices.get(&id).map(|p| SeenPrice {
                        copper: p.copper,
                        kind: p.kind,
                        at: p.at,
                    }),
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
                account_unknown: bank.account_unknown,
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
            account_unknown: bank.account_unknown,
            read_by: bank.read_by.to_string(),
            at: bank.at,
            stacks: bank.list.len(),
            money: bank.money,
        })
        .collect();
    let mut recipes = recipe_rows(lives, names, &q);
    let recipes_matched = recipes.len();
    recipes.truncate(MAX_ROWS);
    Items {
        characters,
        shared,
        rows,
        matched,
        recipes,
        recipes_matched,
    }
}

//! What each character carries: the `BC` lines (docs/FORMAT.md) as the latest bag and bank
//! lists, with the item names the `IN` lines gave. Owner-only: nothing here is uploaded.

use crate::overview::Ev;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Inventory {
    pub bags: BTreeMap<i64, i64>,
    pub bank: BTreeMap<i64, i64>,
    pub bags_at: i64,
    pub bank_at: i64,
    pub names: BTreeMap<i64, String>,
    pub mail: BTreeMap<i64, i64>,
    pub mail_at: i64,
    pub auctions: BTreeMap<i64, i64>,
    pub auctions_at: i64,
    pub warband: BTreeMap<i64, i64>,
    pub warband_at: i64,
    pub guild: BTreeMap<i64, i64>,
    pub guild_at: i64,
    pub guild_name: String,
    pub warband_money: i64,
    pub warband_money_at: i64,
    pub guild_money: i64,
    pub guild_money_at: i64,
    pub guild_money_name: String,
}

impl Inventory {
    pub fn is_empty(&self) -> bool {
        self.bags.is_empty()
            && self.bank.is_empty()
            && self.mail.is_empty()
            && self.auctions.is_empty()
            && self.warband.is_empty()
            && self.guild.is_empty()
            && self.mail_at == 0
            && self.auctions_at == 0
            && self.warband_at == 0
            && self.guild_at == 0
            && self.warband_money_at == 0
            && self.guild_money_at == 0
    }

    pub fn stacks(&self) -> usize {
        self.bags.len()
            + self.bank.len()
            + self.mail.len()
            + self.auctions.len()
            + self.warband.len()
            + self.guild.len()
    }
}

/// The entry with the newest read time for each key; a read time of 0 means never read.
pub fn newest_per<K: Ord, T>(
    reads: impl IntoIterator<Item = (K, i64, T)>,
) -> BTreeMap<K, (i64, T)> {
    let mut newest: BTreeMap<K, (i64, T)> = BTreeMap::new();
    for (key, at, what) in reads {
        if at <= 0 {
            continue;
        }
        match newest.entry(key) {
            Entry::Vacant(slot) => {
                slot.insert((at, what));
            }
            Entry::Occupied(mut slot) => {
                if at > slot.get().0 {
                    slot.insert((at, what));
                }
            }
        }
    }
    newest
}

fn items(list: &str) -> impl Iterator<Item = (i64, i64)> + '_ {
    list.split(',').filter_map(|part| {
        let (id, n) = part.split_once(':')?;
        Some((id.trim().parse().ok()?, n.trim().parse().ok()?))
    })
}

pub fn build<'a>(events: impl IntoIterator<Item = Ev<'a>>) -> Inventory {
    let mut inv = Inventory::default();
    let mut names: BTreeMap<i64, String> = BTreeMap::new();
    let mut last: Option<&str> = None;
    let mut guild = String::new();
    for (ms, code, f) in events {
        let field = |i: usize| f.get(i).map_or("", String::as_str);
        match code {
            "IN" => {
                if let (Some(id), Some(name)) = (
                    f.first().and_then(|s| s.parse::<i64>().ok()),
                    f.get(1).filter(|s| !s.is_empty()),
                ) {
                    names.insert(id, name.clone());
                }
                continue;
            }
            "GU" => match field(0) {
                "s" | "j" | "k" => guild = field(1).to_string(),
                "l" => guild.clear(),
                _ => {}
            },
            "BK" if field(0) == "a" => {
                let Ok(copper) = field(2).parse::<i64>() else {
                    last = None;
                    continue;
                };
                let at = ms.div_euclid(1000);
                match field(1) {
                    "2" => {
                        inv.warband_money = copper;
                        inv.warband_money_at = at;
                    }
                    "1" => {
                        inv.guild_money = copper;
                        inv.guild_money_at = at;
                        inv.guild_money_name.clone_from(&guild);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        if code != "BC" {
            last = None;
            continue;
        }
        let kind = field(0);
        if !matches!(kind, "b" | "k" | "w" | "g" | "m" | "a") {
            continue;
        }
        let (list, at) = match kind {
            "b" => (&mut inv.bags, &mut inv.bags_at),
            "k" => (&mut inv.bank, &mut inv.bank_at),
            "w" => (&mut inv.warband, &mut inv.warband_at),
            "g" => (&mut inv.guild, &mut inv.guild_at),
            "m" => (&mut inv.mail, &mut inv.mail_at),
            _ => (&mut inv.auctions, &mut inv.auctions_at),
        };
        if last != Some(kind) {
            list.clear();
            *at = ms.div_euclid(1000);
            if kind == "g" {
                inv.guild_name.clone_from(&guild);
            }
        }
        last = Some(kind);
        for (id, n) in items(field(1)) {
            *list.entry(id).or_default() += n;
        }
    }
    let wanted: Vec<i64> = [
        &inv.bags,
        &inv.bank,
        &inv.mail,
        &inv.auctions,
        &inv.warband,
        &inv.guild,
    ]
    .into_iter()
    .flat_map(|list| list.keys().copied())
    .collect();
    for id in wanted {
        if let Some(name) = names.remove(&id) {
            inv.names.insert(id, name);
        }
    }
    inv
}

/// The text of Items.lua: every character's lists and the names of the items in them. `chars`
/// is in the order they should be written.
pub fn items_lua(chars: &[(String, Inventory)], made: i64) -> String {
    let mut names: BTreeMap<i64, &str> = BTreeMap::new();
    let mut by_char = serde_json::Map::new();
    for (name, inv) in chars.iter().filter(|(_, inv)| !inv.is_empty()) {
        for (id, n) in &inv.names {
            names.entry(*id).or_insert(n);
        }
        let list = |m: &BTreeMap<i64, i64>| {
            Value::Object(
                m.iter()
                    .map(|(k, v)| (k.to_string(), Value::from(*v)))
                    .collect(),
            )
        };
        let mut c = serde_json::Map::new();
        c.insert("b".into(), list(&inv.bags));
        c.insert("k".into(), list(&inv.bank));
        c.insert("tb".into(), Value::from(inv.bags_at));
        c.insert("tk".into(), Value::from(inv.bank_at));
        by_char.insert(name.clone(), Value::Object(c));
    }
    let mut root = serde_json::Map::new();
    root.insert("v".into(), Value::from(1));
    root.insert("made".into(), Value::from(made));
    root.insert(
        "names".into(),
        Value::Object(
            names
                .into_iter()
                .map(|(k, v)| (k.to_string(), Value::from(v)))
                .collect(),
        ),
    );
    root.insert("chars".into(), Value::Object(by_char));
    let mut out = String::from("ScarletItems = ");
    crate::overview::lua_write(&Value::Object(root), 2, &mut out);
    out.push('\n');
    out
}

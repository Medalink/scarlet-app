//! What each character carries: the `BC` lines (docs/FORMAT.md) as the latest bag and bank
//! lists, with the item names the `IN` lines gave. Owner-only: nothing here is uploaded.

use crate::overview::Ev;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Inventory {
    pub bags: BTreeMap<i64, i64>,
    pub bank: BTreeMap<i64, i64>,
    pub bags_at: i64,
    pub bank_at: i64,
    pub names: BTreeMap<i64, String>,
}

impl Inventory {
    pub fn is_empty(&self) -> bool {
        self.bags.is_empty() && self.bank.is_empty()
    }
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
    for (ms, code, f) in events {
        if code == "IN" {
            if let (Some(id), Some(name)) = (
                f.first().and_then(|s| s.parse::<i64>().ok()),
                f.get(1).filter(|s| !s.is_empty()),
            ) {
                names.insert(id, name.clone());
            }
            continue;
        }
        if code != "BC" {
            last = None;
            continue;
        }
        let Some(kind) = f
            .first()
            .map(String::as_str)
            .filter(|k| *k == "b" || *k == "k")
        else {
            continue;
        };
        let (list, at) = if kind == "b" {
            (&mut inv.bags, &mut inv.bags_at)
        } else {
            (&mut inv.bank, &mut inv.bank_at)
        };
        if last != Some(kind) {
            list.clear();
            *at = ms.div_euclid(1000);
        }
        last = Some(kind);
        for (id, n) in items(f.get(1).map_or("", String::as_str)) {
            *list.entry(id).or_default() += n;
        }
    }
    let wanted: Vec<i64> = inv.bags.keys().chain(inv.bank.keys()).copied().collect();
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

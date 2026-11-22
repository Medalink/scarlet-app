//! The loot view: drop rates per creature and item split by loot rule, and
//! what each player received and rolled. Built from the same `LootBook` the
//! export uses, one per character, since the loot rule is that character's
//! own state.

use crate::commands::{blocking, with_store};
use scarlet::loot::{LootBook, Rule};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use tauri::AppHandle;

/// Codes `LootBook` reads.
const CODES: [&str; 12] = [
    "S", "GP", "LM", "K", "LO", "LW", "O", "LT", "IQ", "RL", "RW", "LD",
];

#[derive(Serialize)]
pub struct RuleOut {
    method: String,
    threshold: String,
    size: u32,
}

impl From<&Rule> for RuleOut {
    fn from(r: &Rule) -> Self {
        RuleOut {
            method: r.method.clone(),
            threshold: r.threshold.clone(),
            size: r.size,
        }
    }
}

#[derive(Serialize)]
pub struct DropOut {
    npc: String,
    npc_name: String,
    item: String,
    item_name: String,
    rule: usize,
    looted: u64,
    drops: u64,
    quantity: u64,
}

#[derive(Serialize)]
pub struct AttrOut {
    who: String,
    rule: usize,
    items: u64,
    quantity: u64,
    quality: [u64; 7],
    rolls_won: u64,
    rolls_lost: u64,
    rolls_passed: u64,
}

#[derive(Serialize)]
pub struct LootView {
    /// Every rule that appears in a row, solo first; rows point into it.
    rules: Vec<RuleOut>,
    drops: Vec<DropOut>,
    attribution: Vec<AttrOut>,
}

#[tauri::command]
pub async fn loot(app: AppHandle, ch: Option<String>) -> LootView {
    blocking(move || loot_sync(app, ch)).await
}

fn loot_sync(app: AppHandle, ch: Option<String>) -> LootView {
    with_store(&app, |st| {
        let only = ch.and_then(|n| st.char_index(&n));
        let mut books: BTreeMap<u32, LootBook> = BTreeMap::new();
        for r in &st.events {
            if only.is_some_and(|c| r.ch != c) || !CODES.contains(&r.code.as_str()) {
                continue;
            }
            books
                .entry(r.ch)
                .or_insert_with(|| LootBook::new(&st.chars[r.ch as usize]))
                .apply(r.ms, &r.code, &r.fields);
        }

        // One row per (creature, item, rule) across the characters' books.
        let mut drops: BTreeMap<(String, String, Rule), DropOut> = BTreeMap::new();
        // One row per (player, rule). A player who is one of your characters
        // is taken from that character's own book only, so a duo that both
        // run Scarlet is not counted twice.
        let mut attr: BTreeMap<(String, Rule), AttrOut> = BTreeMap::new();
        for (ci, book) in &books {
            let me = &st.chars[*ci as usize];
            for d in book.drop_rows(&st.names) {
                let e = drops
                    .entry((d.npc.clone(), d.item.clone(), d.rule.clone()))
                    .or_insert_with(|| DropOut {
                        npc: d.npc.clone(),
                        npc_name: d.npc_name.clone(),
                        item: d.item.clone(),
                        item_name: d.item_name.clone(),
                        rule: 0,
                        looted: 0,
                        drops: 0,
                        quantity: 0,
                    });
                e.looted += d.looted;
                e.drops += d.drops;
                e.quantity += d.quantity;
            }
            for a in book.attribution_rows() {
                // With one character selected only its book is loaded, so it
                // supplies the other characters' rows too.
                if only.is_none() && a.who != *me && st.chars.contains(&a.who) {
                    continue;
                }
                let e = attr
                    .entry((a.who.clone(), a.rule.clone()))
                    .or_insert_with(|| AttrOut {
                        who: a.who.clone(),
                        rule: 0,
                        items: 0,
                        quantity: 0,
                        quality: [0; 7],
                        rolls_won: 0,
                        rolls_lost: 0,
                        rolls_passed: 0,
                    });
                e.items += a.items;
                e.quantity += a.quantity;
                for (i, q) in a.quality.iter().enumerate() {
                    e.quality[i] += q;
                }
                e.rolls_won += a.rolls_won;
                e.rolls_lost += a.rolls_lost;
                e.rolls_passed += a.rolls_passed;
            }
        }

        // Rules in display order: solo first, then by method, threshold, size.
        let mut rules: BTreeSet<Rule> = BTreeSet::new();
        rules.extend(drops.keys().map(|k| k.2.clone()));
        rules.extend(attr.keys().map(|k| k.1.clone()));
        let mut rules: Vec<Rule> = rules.into_iter().collect();
        rules.sort_by_key(|r| (r.method != "solo", r.clone()));
        let at = |r: &Rule| rules.iter().position(|x| x == r).unwrap_or(0);

        let drops_out = drops
            .into_iter()
            .map(|((_, _, rule), mut d)| {
                d.rule = at(&rule);
                d
            })
            .collect();
        let attribution = attr
            .into_iter()
            .map(|((_, rule), mut a)| {
                a.rule = at(&rule);
                a
            })
            .collect();
        LootView {
            rules: rules.iter().map(RuleOut::from).collect(),
            drops: drops_out,
            attribution,
        }
    })
}

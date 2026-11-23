//! The Feats view: the website's count of a character's feats, and the tier of each the player
//! has been shown. Both go through the install's signed requests. With uploading off, or before
//! this install has registered with the website, nothing here makes a network request.

use crate::commands::{blocking, with_store};
use crate::feats_catalog::{self, CatalogFeat};
use crate::recap;
use crate::settings;
use scarlet::featscore::{self, Scored};
use scarlet::upload::config;
use scarlet::upload::keys;
use scarlet::upload::session::Session;
use scarlet::upload::state as ustate;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use tauri::AppHandle;

/// What the page draws from.
#[derive(Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum FeatsState {
    /// Uploading is off, or this install has not registered with the website: the feats the recording can count.
    Local {
        characters: Vec<LocalEntry>,
        account: Option<LocalAccount>,
        catalog: u32,
    },
    /// The website's `characters` list, as it answered.
    Characters {
        characters: Value,
        account: Value,
        catalog: u32,
    },
    Error {
        message: String,
    },
}

#[derive(Serialize)]
pub struct LocalFeat {
    key: &'static str,
    tier: u8,
    value: f64,
    next: Option<f64>,
    left: Option<f64>,
    progress: f64,
}

#[derive(Serialize)]
pub struct LocalEntry {
    name: String,
    class: String,
    spec: String,
    held: usize,
    total: usize,
    elsewhere: usize,
    score: u32,
    score_max: u32,
    tiers: [usize; featscore::TIER_COUNT],
    feats: Vec<LocalFeat>,
    closest: Vec<&'static str>,
}

/// One account crest counted from the characters the app holds: the value over all of them by the feat's
/// `sum` or `max` rule, its tier by the account thresholds, and for `max` the character that holds it.
#[derive(Serialize)]
pub struct LocalAccountFeat {
    key: &'static str,
    tier: u8,
    value: f64,
    prev: f64,
    next: Option<f64>,
    progress: f64,
    holder: Option<String>,
}

#[derive(Serialize)]
pub struct LocalAccount {
    held: usize,
    total: usize,
    tiers: [usize; featscore::TIER_COUNT],
    feats: Vec<LocalAccountFeat>,
}

const CLOSEST: usize = 3;

fn local_scored(st: &crate::store::Store) -> Vec<(String, Scored)> {
    (0..st.chars.len() as u32)
        .filter_map(|ch| {
            let tallies = recap::tallies(st, ch);
            (!tallies.is_empty())
                .then(|| (st.chars[ch as usize].clone(), featscore::score(&tallies)))
        })
        .collect()
}

fn account_feat(
    feat: &'static CatalogFeat,
    scored: &[(String, Scored)],
) -> Option<LocalAccountFeat> {
    let thresholds = feat.account_thresholds?;
    let counts: Vec<(&str, f64)> = scored
        .iter()
        .filter_map(|(name, s)| {
            s.counted
                .iter()
                .find(|c| c.feat.key == feat.key)
                .map(|c| (name.as_str(), c.v))
        })
        .collect();
    if counts.is_empty() {
        return None;
    }
    let (value, holder) = match feat.account {
        "sum" => (counts.iter().map(|(_, v)| v).sum(), None),
        "max" => {
            let (name, best) = counts
                .iter()
                .copied()
                .fold(counts[0], |top, c| if c.1 > top.1 { c } else { top });
            (best, (best > 0.0).then(|| name.to_string()))
        }
        _ => return None,
    };
    let tier = thresholds.iter().filter(|t| value >= **t).count();
    let prev = if tier == 0 { 0.0 } else { thresholds[tier - 1] };
    let next = thresholds.get(tier).copied();
    let progress = next.map_or(1.0, |n| ((value - prev) / (n - prev)).clamp(0.0, 1.0));
    Some(LocalAccountFeat {
        key: feat.key,
        tier: tier as u8,
        value,
        prev,
        next,
        progress,
        holder,
    })
}

fn local_account(scored: &[(String, Scored)]) -> Option<LocalAccount> {
    let feats: Vec<LocalAccountFeat> = feats_catalog::FEATS
        .iter()
        .filter(|f| f.account != "none")
        .filter_map(|f| account_feat(f, scored))
        .collect();
    if feats.is_empty() {
        return None;
    }
    let mut tiers = [0; featscore::TIER_COUNT];
    for f in feats.iter().filter(|f| f.tier > 0) {
        tiers[f.tier as usize - 1] += 1;
    }
    Some(LocalAccount {
        held: feats.iter().filter(|f| f.tier > 0).count(),
        total: feats.len(),
        tiers,
        feats,
    })
}

fn local_entries(scored: Vec<(String, Scored)>, only: Option<&str>) -> Vec<LocalEntry> {
    scored
        .into_iter()
        .filter(|(name, _)| only.is_none_or(|wanted| name == wanted))
        .map(|(name, scored)| LocalEntry {
            name,
            held: scored.held(),
            total: scored.total(),
            elsewhere: scored.elsewhere(),
            score: scored.score(),
            score_max: scored.score_max(),
            tiers: scored.tier_counts(),
            closest: scored.closest(CLOSEST).iter().map(|c| c.feat.key).collect(),
            feats: scored
                .counted
                .iter()
                .map(|c| LocalFeat {
                    key: c.feat.key,
                    tier: c.tier,
                    value: c.v,
                    next: c.next(),
                    left: c.left(),
                    progress: c.progress(),
                })
                .collect(),
            class: scored.class,
            spec: scored.spec,
        })
        .collect()
}

pub(crate) fn registered_session() -> Result<Option<Session>, String> {
    let s = settings::load();
    let cfg = &s.upload;
    if !cfg.enabled {
        return Ok(None);
    }
    if let Some(p) = cfg.server_problem() {
        return Err(p);
    }
    let registered = ustate::load(&s.data)
        .installs
        .contains_key(&config::site(&cfg.server));
    if !registered {
        return Ok(None);
    }
    Session::open(cfg, &s.data, keys::system_store().as_ref(), false).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn feats_get(app: AppHandle, ch: Option<String>) -> FeatsState {
    blocking(move || {
        let mut sess = match registered_session() {
            Ok(Some(sess)) => sess,
            Ok(None) => {
                let (characters, account) = with_store(&app, |st| {
                    let scored = local_scored(st);
                    let account = local_account(&scored);
                    (local_entries(scored, ch.as_deref()), account)
                });
                return FeatsState::Local {
                    characters,
                    account,
                    catalog: feats_catalog::VERSION,
                };
            }
            Err(message) => return FeatsState::Error { message },
        };
        match sess.feats() {
            Ok(v) => FeatsState::Characters {
                characters: Value::Array(
                    v.get("characters")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter(|c| ch.as_deref().is_none_or(|name| c["name"] == name))
                        .cloned()
                        .collect(),
                ),
                account: v.get("account").cloned().unwrap_or(Value::Null),
                catalog: feats_catalog::VERSION,
            },
            Err(e) => FeatsState::Error {
                message: e.to_string(),
            },
        }
    })
    .await
}

/// Tell the website which tier of each feat (by key) the player was just shown for `character`, or for the account.
#[tauri::command]
pub async fn feats_seen(
    character: String,
    shown: BTreeMap<String, u8>,
    account: Option<bool>,
) -> Result<(), String> {
    if shown.is_empty() {
        return Ok(());
    }
    blocking(move || match registered_session()? {
        Some(mut sess) => {
            let sent = if account == Some(true) {
                sess.feats_seen_account(&shown)
            } else {
                sess.feats_seen(&character, &shown)
            };
            sent.map_err(|e| e.to_string())
        }
        None => Ok(()),
    })
    .await
}

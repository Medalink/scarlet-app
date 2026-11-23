//! The Feats view: the website's count of a character's feats, and the tier of each the player
//! has been shown. Both go through the install's signed requests. With uploading off, or before
//! this install has registered with the website, nothing here makes a network request.

use crate::commands::{blocking, with_store};
use crate::feats_catalog;
use crate::recap;
use crate::settings;
use scarlet::featscore;
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
        catalog: u32,
    },
    /// The website's `characters` list, as it answered.
    Characters {
        characters: Value,
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

const CLOSEST: usize = 3;

fn local_entries(st: &crate::store::Store, only: Option<&str>) -> Vec<LocalEntry> {
    (0..st.chars.len() as u32)
        .filter(|ch| only.is_none_or(|name| st.chars[*ch as usize] == name))
        .filter_map(|ch| {
            let tallies = recap::tallies(st, ch);
            if tallies.is_empty() {
                return None;
            }
            let scored = featscore::score(&tallies);
            Some(LocalEntry {
                name: st.chars[ch as usize].clone(),
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
                return FeatsState::Local {
                    characters: with_store(&app, |st| local_entries(st, ch.as_deref())),
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
                catalog: feats_catalog::VERSION,
            },
            Err(e) => FeatsState::Error {
                message: e.to_string(),
            },
        }
    })
    .await
}

/// Tell the website which tier of each feat (by key) the player was just shown for `character`.
#[tauri::command]
pub async fn feats_seen(character: String, shown: BTreeMap<String, u8>) -> Result<(), String> {
    if shown.is_empty() {
        return Ok(());
    }
    blocking(move || match registered_session()? {
        Some(mut sess) => sess
            .feats_seen(&character, &shown)
            .map_err(|e| e.to_string()),
        None => Ok(()),
    })
    .await
}

//! The Feats view: the website's count of a character's feats, and the tier of each the player
//! has been shown. Both go through the install's signed requests. With uploading off, or before
//! this install has registered with the website, nothing here makes a network request.

use crate::settings;
use scarlet::upload::config;
use scarlet::upload::keys;
use scarlet::upload::session::Session;
use scarlet::upload::state as ustate;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// What the page draws from.
#[derive(Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum FeatsState {
    /// Uploading is off, or this install has not registered with the website.
    Off,
    /// The website's `characters` list, as it answered.
    Characters {
        characters: Value,
    },
    Error {
        message: String,
    },
}

fn registered_session() -> Result<Option<Session>, String> {
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

#[tauri::command(async)]
pub fn feats_get() -> FeatsState {
    let mut sess = match registered_session() {
        Ok(Some(sess)) => sess,
        Ok(None) => return FeatsState::Off,
        Err(message) => return FeatsState::Error { message },
    };
    match sess.feats() {
        Ok(v) => FeatsState::Characters {
            characters: v.get("characters").cloned().unwrap_or(Value::Array(vec![])),
        },
        Err(e) => FeatsState::Error {
            message: e.to_string(),
        },
    }
}

/// Tell the website which tier of each feat (by key) the player was just shown for `character`.
#[tauri::command(async)]
pub fn feats_seen(character: String, shown: BTreeMap<String, u8>) -> Result<(), String> {
    if shown.is_empty() {
        return Ok(());
    }
    match registered_session()? {
        Some(mut sess) => sess
            .feats_seen(&character, &shown)
            .map_err(|e| e.to_string()),
        None => Ok(()),
    }
}

//! The `upload` object of the app's `settings.json`, shared by the app and the CLI.

use super::{Error, Result};
use scarlet_part::tables::Groups;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

/// The Scarlet website. A settings file with no address, or an empty one, uses it.
pub const DEFAULT_SERVER: &str = "https://scarletweb.io";

fn default_server() -> String {
    DEFAULT_SERVER.into()
}

fn server_or_default<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<String, D::Error> {
    let s = String::deserialize(d)?;
    Ok(if s.trim().is_empty() {
        default_server()
    } else {
        s
    })
}

/// What the player chose. Everything is off until `enabled` is turned on; a group or
/// character that is not listed is on.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UploadConfig {
    #[serde(default)]
    pub enabled: bool,
    /// The website's address; `DEFAULT_SERVER` unless the player typed another.
    #[serde(default = "default_server", deserialize_with = "server_or_default")]
    pub server: String,
    /// Sharing group key → on. Only groups that were switched are listed.
    #[serde(default)]
    pub groups: BTreeMap<String, bool>,
    /// `Name-Surname` → on.
    #[serde(default)]
    pub characters: BTreeMap<String, bool>,
}

/// A sharing group as the settings page shows it.
#[derive(Serialize, Clone, Debug)]
pub struct GroupInfo {
    pub key: String,
    pub label: String,
    /// `core` and `names` cannot be switched off.
    pub switchable: bool,
}

/// Every group in `groups.tsv` order.
pub fn all_groups() -> Vec<GroupInfo> {
    Groups::get()
        .list
        .iter()
        .map(|g| GroupInfo {
            key: g.key.clone(),
            label: g.label.clone(),
            switchable: g.switchable,
        })
        .collect()
}

pub fn is_group(key: &str) -> bool {
    Groups::get().index(key).is_some()
}

pub fn is_switchable(key: &str) -> bool {
    Groups::get()
        .list
        .iter()
        .any(|g| g.key == key && g.switchable)
}

impl Default for UploadConfig {
    fn default() -> Self {
        UploadConfig {
            enabled: false,
            server: default_server(),
            groups: BTreeMap::new(),
            characters: BTreeMap::new(),
        }
    }
}

impl UploadConfig {
    pub fn group_on(&self, key: &str) -> bool {
        !is_switchable(key) || self.groups.get(key).copied().unwrap_or(true)
    }

    pub fn character_on(&self, name: &str) -> bool {
        self.characters.get(name).copied().unwrap_or(true)
    }

    /// The switchable groups that are on.
    pub fn on_groups(&self) -> BTreeSet<String> {
        Groups::get()
            .list
            .iter()
            .filter(|g| g.switchable && self.group_on(&g.key))
            .map(|g| g.key.clone())
            .collect()
    }

    /// Group keys for the API: `core`, `names` and the groups that are on, in `groups.tsv` order.
    pub fn api_groups(&self) -> Vec<String> {
        Groups::get()
            .list
            .iter()
            .filter(|g| self.group_on(&g.key))
            .map(|g| g.key.clone())
            .collect()
    }

    /// Why uploading cannot be turned on, when it cannot.
    pub fn server_problem(&self) -> Option<String> {
        server_problem(&self.server)
    }
}

/// The site's address must be https, or http on this PC.
pub fn server_problem(server: &str) -> Option<String> {
    let s = server.trim();
    if s.is_empty() {
        return Some("The Scarlet website has no public address yet.".into());
    }
    let Some((scheme, rest)) = s.split_once("://") else {
        return Some("The address must start with https://.".into());
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.rsplit('@').next().unwrap_or("");
    let name = if let Some(v6) = host.strip_prefix('[') {
        v6.split(']').next().unwrap_or("")
    } else {
        host.split(':').next().unwrap_or("")
    };
    if name.is_empty() {
        return Some("The address has no host name.".into());
    }
    match scheme {
        "https" => None,
        "http" if matches!(name, "localhost" | "127.0.0.1" | "::1") => None,
        "http" => Some("The address must use https, except on this PC.".into()),
        _ => Some("The address must start with https://.".into()),
    }
}

/// The address without a trailing slash.
pub fn site(server: &str) -> String {
    server.trim().trim_end_matches('/').to_string()
}

/// The `upload` object of a settings file; the defaults when the file or the object is missing.
pub fn load(settings: &Path) -> Result<UploadConfig> {
    let text = match fs::read_to_string(settings) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(UploadConfig::default()),
        Err(e) => return Err(Error::local(format!("{}: {e}", settings.display()))),
    };
    let v: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| Error::local(format!("{}: {e}", settings.display())))?;
    match v.get("upload") {
        Some(u) => serde_json::from_value(u.clone())
            .map_err(|e| Error::local(format!("{}: upload: {e}", settings.display()))),
        None => Ok(UploadConfig::default()),
    }
}

/// Change the `upload` object and leave every other key of the settings file as it is. Written
/// beside the file and renamed over it.
pub fn update(settings: &Path, f: impl FnOnce(&mut UploadConfig)) -> Result<UploadConfig> {
    let mut root: serde_json::Value = match fs::read_to_string(settings) {
        Ok(t) => serde_json::from_str(&t)
            .map_err(|e| Error::local(format!("{}: {e}", settings.display())))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => serde_json::json!({}),
        Err(e) => return Err(Error::local(format!("{}: {e}", settings.display()))),
    };
    let mut cfg = load(settings)?;
    f(&mut cfg);
    let obj = root
        .as_object_mut()
        .ok_or_else(|| Error::local("settings.json is not an object"))?;
    obj.insert(
        "upload".into(),
        serde_json::to_value(&cfg).map_err(|e| Error::local(e.to_string()))?,
    );
    if let Some(dir) = settings.parent() {
        fs::create_dir_all(dir).map_err(|e| Error::local(e.to_string()))?;
    }
    let tmp = settings.with_extension("json.tmp");
    let mut json = serde_json::to_string_pretty(&root).map_err(|e| Error::local(e.to_string()))?;
    json.push('\n');
    fs::write(&tmp, json).map_err(|e| Error::local(e.to_string()))?;
    fs::rename(&tmp, settings).map_err(|e| Error::local(e.to_string()))?;
    Ok(cfg)
}

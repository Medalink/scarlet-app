//! `<out>/upload-state.json`: what the server acknowledged, as part ids, and when and how much
//! was sent. No part text and no key. Written through a temp file and a rename.

use super::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct CharState {
    /// Ids of the parts the server held after the last sync, sorted.
    #[serde(default)]
    pub acked: Vec<String>,
    /// Epoch seconds of the last sync that finished.
    #[serde(default)]
    pub last_sync: u64,
    /// Bytes sent and received by the last sync that sent something, and in all.
    #[serde(default)]
    pub last_sent: u64,
    #[serde(default)]
    pub last_received: u64,
    #[serde(default)]
    pub total_sent: u64,
    #[serde(default)]
    pub total_received: u64,
    /// Sessions the server holds parts of, from the last sync.
    #[serde(default)]
    pub sessions: u64,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct UploadState {
    #[serde(default)]
    pub v: u32,
    /// Install id per server address.
    #[serde(default)]
    pub installs: BTreeMap<String, String>,
    #[serde(default)]
    pub characters: BTreeMap<String, CharState>,
}

pub fn path(out: &Path) -> PathBuf {
    out.join("upload-state.json")
}

pub fn load(out: &Path) -> UploadState {
    fs::read_to_string(path(out))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save(out: &Path, state: &UploadState) -> Result<()> {
    let p = path(out);
    fs::create_dir_all(out).map_err(|e| Error::local(e.to_string()))?;
    let tmp = p.with_extension("json.tmp");
    let mut s = state.clone();
    s.v = 1;
    let json = serde_json::to_string(&s).map_err(|e| Error::local(e.to_string()))?;
    fs::write(&tmp, json + "\n").map_err(|e| Error::local(e.to_string()))?;
    fs::rename(&tmp, &p).map_err(|e| Error::local(e.to_string()))
}

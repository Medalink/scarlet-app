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
    /// The last uploads, oldest first, at most `LOG_KEEP`.
    #[serde(default)]
    pub log: Vec<LogEntry>,
}

/// Uploads kept in the log.
pub const LOG_KEEP: usize = 50;

/// One upload run: what it sent, or why it stopped.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct LogEntry {
    /// Epoch seconds the run ended.
    pub at: u64,
    #[serde(default)]
    pub parts: u64,
    #[serde(default)]
    pub sessions: u64,
    /// Bytes sent, requests included.
    #[serde(default)]
    pub sent: u64,
    /// The characters that sent parts.
    #[serde(default)]
    pub characters: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Later runs folded into this one because they sent nothing and ended the same way.
    #[serde(default)]
    pub repeats: u32,
}

impl UploadState {
    /// Add a run to the log. A run that sent nothing and ended like the one before it moves that
    /// entry's time forward instead of adding a line.
    pub fn log(&mut self, e: LogEntry) {
        if let Some(last) = self.log.last_mut()
            && last.parts == 0
            && e.parts == 0
            && last.error == e.error
        {
            last.at = e.at;
            last.sent += e.sent;
            last.repeats += 1;
            return;
        }
        self.log.push(e);
        let n = self.log.len();
        if n > LOG_KEEP {
            self.log.drain(..n - LOG_KEEP);
        }
    }
}

/// Add a run to the log on disk, for a run that stopped before it had a session.
pub fn record(out: &Path, e: LogEntry) -> Result<()> {
    let mut s = load(out);
    s.log(e);
    save(out, &s)
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
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
    let mut json = serde_json::to_string(&s).map_err(|e| Error::local(e.to_string()))?;
    json.push('\n');
    fs::write(&tmp, json).map_err(|e| Error::local(e.to_string()))?;
    fs::rename(&tmp, &p).map_err(|e| Error::local(e.to_string()))
}

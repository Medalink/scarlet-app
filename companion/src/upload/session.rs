//! One connection to the website: registration, the sync of each character, and the removals.
//! Follows "One upload, start to end" in docs/API.md.

use super::api::{Client, Hit, Reply};
use super::config::{self, UploadConfig};
use super::keys::{self, KeyStore};
use super::state::{self, CharState, LogEntry, UploadState, now};
use super::{Error, Result};
use scarlet_part::build::{Built, Options, build_parts};
use scarlet_part::imports;
use scarlet_part::load::{load_dir, load_imports};
use scarlet_part::pseudo::hex;
use scarlet_part::wire;
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashMap};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

const MANIFEST_TYPE: &str = "application/vnd.scarlet.manifest+zstd";
const PARTS_TYPE: &str = "application/vnd.scarlet.parts+zstd";

/// What one character's sync did. No part text, nothing but counts.
#[derive(Clone, Debug, Default)]
pub struct CharReport {
    pub name: String,
    /// Parts the app holds for the character in the groups that are on.
    pub parts: usize,
    /// True when the one compare request was all it took.
    pub matched: bool,
    pub parts_sent: usize,
    pub sessions_sent: usize,
    pub requests: usize,
    pub sent: u64,
    pub received: u64,
    pub ms: u64,
    /// Groups the server refused; they are off now and the caller keeps them off.
    pub refused: Vec<String>,
    /// Why nothing was done or what went wrong.
    pub note: Option<String>,
    pub failed: bool,
    /// The failure may pass: no answer, a 5xx or a 429. A refusal or a local fault will not.
    pub retry: bool,
}

#[derive(Clone, Debug)]
pub struct LinkInfo {
    pub code: String,
    pub url: String,
    pub expires_at: String,
}

#[derive(Clone, Debug)]
pub struct Me {
    pub linked: bool,
    pub characters: u64,
}

pub struct Session {
    pub cfg: UploadConfig,
    out: PathBuf,
    client: Client,
    name_key: [u8; 32],
    state: UploadState,
}

/// Character folders under the data folder: those with a `payloads` folder.
pub fn list_characters(out: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(out)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().join("payloads").is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

impl Session {
    /// Connect with this PC's install identity. With `create` unset and no identity yet, there
    /// is nothing on the server to speak of and `None` comes back.
    pub fn open(
        cfg: &UploadConfig,
        out: &Path,
        store: &dyn KeyStore,
        create: bool,
    ) -> Result<Option<Session>> {
        if let Some(p) = cfg.server_problem() {
            return Err(Error::local(p));
        }
        let Some((id, fresh)) = keys::identity(store, create)? else {
            return Ok(None);
        };
        let mut state = state::load(out);
        if fresh {
            // Install ids remembered for an earlier key do not belong to this one.
            state.installs.clear();
        }
        let mut s = Session {
            cfg: cfg.clone(),
            out: out.to_path_buf(),
            client: Client::new(&cfg.server, Some(&id.seed)),
            name_key: id.name_key,
            state,
        };
        s.ensure_registered(false)?;
        Ok(Some(s))
    }

    fn site(&self) -> String {
        self.client.site().to_string()
    }

    fn ensure_registered(&mut self, force: bool) -> Result<()> {
        let site = self.site();
        if !force && let Some(id) = self.state.installs.get(&site) {
            let id = id.clone();
            self.client.set_install(&id);
            return Ok(());
        }
        let reply = self.client.register("Scarlet app")?;
        let id = reply
            .json
            .pointer("/install/id")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::local("the server did not answer with an install id"))?
            .to_string();
        self.client.set_install(&id);
        self.state.installs.insert(site, id);
        state::save(&self.out, &self.state)
    }

    /// A signed request; an install the server does not know (its database was reset) is
    /// registered again once, which is a new request and not a retry of the refused one.
    fn api(
        &mut self,
        method: &'static str,
        path: &str,
        body: Option<(&str, &[u8])>,
    ) -> Result<Reply> {
        let first = self.call(method, path, body);
        if matches!(&first, Err(e) if e.code() == Some("unknown_install")) {
            self.ensure_registered(true)?;
            return self.call(method, path, body);
        }
        first
    }

    fn call(
        &mut self,
        method: &'static str,
        path: &str,
        body: Option<(&str, &[u8])>,
    ) -> Result<Reply> {
        match body {
            Some((ct, b)) => self.client.call_body(method, path, ct, b),
            None => self.client.call(method, path),
        }
    }

    /// Requests made so far, oldest first.
    pub fn hits(&self) -> &[Hit] {
        &self.client.hits
    }

    pub fn state(&self) -> &UploadState {
        &self.state
    }

    fn build(&self, name: &str) -> Result<(String, Built)> {
        let dir = self.out.join(name);
        let loaded = load_dir(
            &dir.join("payloads"),
            Some(&dir.join("addons").join("payloads")),
        )
        .map_err(Error::local)?;
        let opts = Options {
            on: self.cfg.on_groups(),
            name_key: self.name_key,
        };
        let character = loaded.character.clone();
        let mut built = build_parts(&character, loaded.main, loaded.addon, &opts);
        let imported = load_imports(&dir.join("imports").join("payloads"), &character);
        built
            .parts
            .extend(imports::build_parts(&character, imported, &opts));
        Ok((character, built))
    }

    /// Sync every character with uploading on, or only `only`.
    pub fn sync_all(&mut self, only: Option<&str>) -> Vec<CharReport> {
        let mut reports = Vec::new();
        for name in list_characters(&self.out) {
            if only.is_some_and(|o| o != name) || !self.cfg.character_on(&name) {
                continue;
            }
            let start = self.client.hits.len();
            let t = Instant::now();
            let mut r = CharReport {
                name: name.clone(),
                ..CharReport::default()
            };
            if let Err(e) = self.sync_character(&name, &mut r) {
                r.failed = true;
                r.retry = e.transient();
                r.note = Some(e.to_string());
            }
            r.ms = t.elapsed().as_millis() as u64;
            let hits = &self.client.hits[start..];
            r.requests = hits.len();
            r.sent = hits.iter().map(Hit::sent).sum();
            r.received = hits.iter().map(Hit::received).sum();
            if r.sent > 0 {
                let st = self.state.characters.entry(r.name.clone()).or_default();
                st.last_sent = r.sent;
                st.last_received = r.received;
                st.total_sent += r.sent;
                st.total_received += r.received;
                let _ = state::save(&self.out, &self.state);
            }
            reports.push(r);
        }
        let sent: Vec<&CharReport> = reports.iter().filter(|r| r.parts_sent > 0).collect();
        self.state.log(LogEntry {
            at: now(),
            parts: sent.iter().map(|r| r.parts_sent as u64).sum(),
            sessions: sent.iter().map(|r| r.sessions_sent as u64).sum(),
            sent: reports.iter().map(|r| r.sent).sum(),
            characters: sent.iter().map(|r| r.name.clone()).collect(),
            error: reports
                .iter()
                .find(|r| r.failed)
                .and_then(|r| r.note.clone()),
            repeats: 0,
        });
        let _ = state::save(&self.out, &self.state);
        reports
    }

    fn sync_character(&mut self, name: &str, report: &mut CharReport) -> Result<()> {
        for _ in 0..2 {
            let (character, built) = self.build(name)?;
            report.name = character.clone();
            report.parts = built.parts.len();
            let held = self
                .state
                .characters
                .get(&character)
                .is_some_and(|s| !s.acked.is_empty());
            if built.parts.is_empty() && !held {
                report.matched = true;
                report.note = Some("no finished session yet".into());
                return Ok(());
            }
            match self.run(&character, &built, report) {
                Err(Error::Api { code, extra, .. }) if code == "group_refused" => {
                    let groups: Vec<String> = extra
                        .get("groups")
                        .and_then(Value::as_array)
                        .map(|a| {
                            a.iter()
                                .filter_map(|g| g.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default();
                    if groups.is_empty() {
                        return Err(Error::local("the server refused a group it did not name"));
                    }
                    for g in groups {
                        self.cfg.groups.insert(g.clone(), false);
                        report.refused.push(g);
                    }
                }
                other => return other,
            }
        }
        Err(Error::local("the server kept refusing groups"))
    }

    fn run(&mut self, character: &str, built: &Built, report: &mut CharReport) -> Result<()> {
        let groups = self.cfg.api_groups();
        let ids: Vec<String> = built.parts.iter().map(|p| hex(&p.id)).collect();
        let digest = hex(&wire::set_digest(built.parts.iter().map(|p| &p.id)));
        let sync = json!({
            "character": character,
            "groups": groups,
            "count": built.parts.len(),
            "digest": digest,
        })
        .to_string();
        let reply = self.api(
            "POST",
            "/upload/sync",
            Some(("application/json", sync.as_bytes())),
        )?;
        if reply.json.get("match").and_then(Value::as_bool) == Some(true) {
            report.matched = true;
            return self.finish(character, built, &ids);
        }

        let entries: Vec<Value> = built
            .parts
            .iter()
            .zip(&ids)
            .map(|(p, id)| {
                json!({"id": id, "session": p.session, "group": p.group, "lines": p.lines, "raw_bytes": p.text.len()})
            })
            .collect();
        let manifest =
            json!({"character": character, "groups": groups, "parts": entries}).to_string();
        let frame = zstd_frame(manifest.as_bytes())?;
        let reply = self.api("POST", "/upload/manifest", Some((MANIFEST_TYPE, &frame)))?;
        let need: Vec<&str> = reply
            .json
            .get("need")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();

        let at: HashMap<&str, usize> = ids
            .iter()
            .enumerate()
            .map(|(i, id)| (id.as_str(), i))
            .collect();
        let mut picked: Vec<usize> = Vec::new();
        for id in &need {
            picked.push(*at.get(id).ok_or_else(|| {
                Error::local("the server asked for a part this PC does not hold")
            })?);
        }
        let texts: Vec<&str> = picked
            .iter()
            .map(|&i| built.parts[i].text.as_str())
            .collect();
        let sessions: BTreeSet<i64> = picked.iter().map(|&i| built.parts[i].session).collect();
        for (body, idx) in wire::split_requests(&texts).map_err(Error::local)? {
            let reply = self.api("POST", "/upload/parts", Some((PARTS_TYPE, &body)))?;
            let stored = reply
                .json
                .get("stored")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            if stored != idx.len() {
                return Err(Error::local("the server stored fewer parts than were sent"));
            }
            report.parts_sent += idx.len();
        }
        report.sessions_sent = sessions.len();
        self.finish(character, built, &ids)
    }

    /// The server now holds exactly these parts: remember them.
    fn finish(&mut self, character: &str, built: &Built, ids: &[String]) -> Result<()> {
        let mut acked = ids.to_vec();
        acked.sort();
        let sessions = built
            .parts
            .iter()
            .map(|p| p.session)
            .collect::<BTreeSet<_>>()
            .len() as u64;
        let st: &mut CharState = self
            .state
            .characters
            .entry(character.to_string())
            .or_default();
        st.acked = acked;
        st.last_sync = now();
        st.sessions = sessions;
        state::save(&self.out, &self.state)
    }

    /// `DELETE /upload/groups/{key}`: the server drops that group and refuses it until it is
    /// accepted again. Returns the answer's `parts_removed` and `rows_removed`.
    pub fn remove_group(&mut self, key: &str) -> Result<(u64, u64)> {
        check_group(key)?;
        let r = self.api("DELETE", &format!("/upload/groups/{key}"), None)?;
        for st in self.state.characters.values_mut() {
            st.acked.clear();
        }
        state::save(&self.out, &self.state)?;
        let n = |k: &str| r.json.get(k).and_then(Value::as_u64).unwrap_or(0);
        Ok((n("parts_removed"), n("rows_removed")))
    }

    /// `PUT /upload/groups/{key}`: the server accepts the group again.
    pub fn restore_group(&mut self, key: &str) -> Result<()> {
        check_group(key)?;
        self.api("PUT", &format!("/upload/groups/{key}"), None)?;
        Ok(())
    }

    /// `DELETE /upload`: everything this install's owner uploaded. Returns the answer's counts.
    pub fn delete_all(&mut self) -> Result<(u64, u64, u64)> {
        let r = self.api("DELETE", "/upload", None)?;
        self.state.characters.clear();
        state::save(&self.out, &self.state)?;
        let n = |k: &str| r.json.get(k).and_then(Value::as_u64).unwrap_or(0);
        Ok((
            n("characters_removed"),
            n("parts_removed"),
            n("rows_removed"),
        ))
    }

    /// A link code, and the page to confirm it on.
    pub fn link(&mut self) -> Result<LinkInfo> {
        let r = self.api("POST", "/installs/link-code", None)?;
        let text = |k: &str| {
            r.json
                .get(k)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        };
        let code = text("code");
        Ok(LinkInfo {
            url: format!("{}/link?code={code}", self.site()),
            code,
            expires_at: text("expires_at"),
        })
    }

    pub fn me(&mut self) -> Result<Me> {
        let r = self.api("GET", "/installs/me", None)?;
        Ok(Me {
            linked: r
                .json
                .get("linked")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            characters: r
                .json
                .get("characters")
                .and_then(Value::as_u64)
                .unwrap_or(0),
        })
    }

    /// `GET /upload/status`, as the server answered.
    pub fn server_status(&mut self) -> Result<Value> {
        Ok(self.api("GET", "/upload/status", None)?.json)
    }
}

fn check_group(key: &str) -> Result<()> {
    if !config::is_switchable(key) {
        return Err(Error::local(format!(
            "'{key}' is not a group that can be switched off"
        )));
    }
    Ok(())
}

/// One zstd frame, level 19, window 8 MiB: what the protocol reads.
fn zstd_frame(raw: &[u8]) -> Result<Vec<u8>> {
    let err = |e: std::io::Error| Error::local(e.to_string());
    let mut enc = zstd::stream::write::Encoder::new(Vec::new(), wire::LEVEL).map_err(err)?;
    enc.window_log(wire::WINDOW_LOG).map_err(err)?;
    enc.set_pledged_src_size(Some(raw.len() as u64))
        .map_err(err)?;
    enc.write_all(raw).map_err(err)?;
    enc.finish().map_err(err)
}

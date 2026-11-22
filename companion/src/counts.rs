//! Counts other addons keep, as `YC` lines (Scarlet's FORMAT.md, "Addon counts"). Each count an addon
//! reads is kept in a per-character archive of the addon's own fields; the upload payload is rebuilt
//! from the archive on every export, mapping those fields to Scarlet's metric names.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub(crate) const DIR: &str = "counts";
const ARCHIVE_MAGIC: &str = "scarletcountsarchive";
pub(crate) const PAYLOAD_MAGIC: &str = "scarletcounts";
const DAY_S: i64 = 86_400;

pub const FISHING_LOG: &str = "fl";
pub const FOREVER_FISHING: &str = "ff";
pub const MASTERCRAFT: &str = "mc";
pub const SESSION: &str = "session";
pub const LIFE: &str = "life";
pub const CHARACTER: &str = "c";
pub const ACCOUNT: &str = "a";
pub const FISHING: &str = "fishing";

/// One statement an addon makes, with the addon's own numeric fields as it wrote them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub origin: String,
    pub store: String,
    pub scope: String,
    pub window: String,
    pub kind: String,
    pub dims: String,
    pub from: Option<i64>,
    pub to: i64,
    pub fields: BTreeMap<String, i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    to: i64,
    origin: String,
    store: String,
    scope: String,
    window: String,
    kind: String,
    dims: String,
    life_from: Option<i64>,
}

impl Record {
    fn key(&self) -> Key {
        Key {
            to: self.to,
            origin: self.origin.clone(),
            store: self.store.clone(),
            scope: self.scope.clone(),
            window: self.window.clone(),
            kind: self.kind.clone(),
            dims: self.dims.clone(),
            life_from: self.from.filter(|_| self.window == LIFE),
        }
    }

    fn same_series(&self, other: &Record) -> bool {
        self.origin == other.origin
            && self.store == other.store
            && self.scope == other.scope
            && self.window == other.window
            && self.kind == other.kind
            && self.dims == other.dims
            && self.from == other.from
    }

    fn line(&self) -> String {
        let fields: Vec<String> = self
            .fields
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        [
            self.origin.clone(),
            self.store.clone(),
            self.scope.clone(),
            self.window.clone(),
            self.kind.clone(),
            self.dims.clone(),
            self.to.to_string(),
            self.from.map(|f| f.to_string()).unwrap_or_default(),
            fields.join(";"),
        ]
        .join("\t")
    }

    fn parse(line: &str) -> Option<Record> {
        let f: Vec<&str> = line.split('\t').collect();
        let &[origin, store, scope, window, kind, dims, to, from, fields] = f.as_slice() else {
            return None;
        };
        let mut parsed = BTreeMap::new();
        for pair in fields.split(';').filter(|p| !p.is_empty()) {
            let (name, value) = pair.split_once('=')?;
            parsed.insert(name.to_string(), value.parse().ok()?);
        }
        Some(Record {
            origin: origin.to_string(),
            store: store.to_string(),
            scope: scope.to_string(),
            window: window.to_string(),
            kind: kind.to_string(),
            dims: dims.to_string(),
            to: to.parse().ok()?,
            from: match from {
                "" => None,
                n => Some(n.parse().ok()?),
            },
            fields: parsed,
        })
    }
}

/// Every record a character's addons have ever shown, one per key.
#[derive(Default)]
pub struct Archive {
    records: BTreeMap<Key, Record>,
}

fn archive_path(char_dir: &Path) -> PathBuf {
    char_dir.join(DIR).join("archive.tsv")
}

fn payload_path(char_dir: &Path) -> PathBuf {
    char_dir.join(DIR).join("payloads").join("counts.scl")
}

fn utc_day(at: i64) -> i64 {
    at.div_euclid(DAY_S)
}

impl Archive {
    /// A character's archive, empty when it has none yet. A line that does not read is an error, so
    /// nothing is written over records this reader does not understand.
    pub fn load(char_dir: &Path) -> Result<Archive, String> {
        let path = archive_path(char_dir);
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Archive::default()),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        let mut lines = text.lines();
        let head: Vec<&str> = lines.next().unwrap_or("").split('\t').collect();
        if head.first() != Some(&ARCHIVE_MAGIC) || head.get(1) != Some(&"1") {
            return Err(format!(
                "{}: line 1 is not an archive header",
                path.display()
            ));
        }
        let mut records = BTreeMap::new();
        for (i, line) in lines.enumerate() {
            let Some(rec) = Record::parse(line) else {
                return Err(format!("{}: line {} does not read", path.display(), i + 2));
            };
            records.insert(rec.key(), rec);
        }
        Ok(Archive { records })
    }

    /// Adds what was read. A key read again with other values replaces its record; nothing is
    /// removed. A `life` total joins only once a UTC day and only when it differs from the latest
    /// one of its series. Returns how many records were added or replaced.
    pub fn merge(&mut self, read: Vec<Record>) -> usize {
        let mut changed = 0;
        for rec in read {
            if rec.window == LIFE && !self.admits_life(&rec) {
                continue;
            }
            let key = rec.key();
            if self.records.get(&key) == Some(&rec) {
                continue;
            }
            self.records.insert(key, rec);
            changed += 1;
        }
        changed
    }

    fn admits_life(&self, rec: &Record) -> bool {
        if let Some(old) = self.records.get(&rec.key()) {
            return old != rec;
        }
        let mut latest: Option<&Record> = None;
        for old in self.records.values().filter(|o| o.same_series(rec)) {
            if utc_day(old.to) == utc_day(rec.to) {
                return false;
            }
            if latest.is_none_or(|l| old.to > l.to) {
                latest = Some(old);
            }
        }
        latest.is_none_or(|l| l.fields != rec.fields)
    }

    fn text(&self) -> String {
        let mut out = format!("{ARCHIVE_MAGIC}\t1\n");
        for rec in self.records.values() {
            out.push_str(&rec.line());
            out.push('\n');
        }
        out
    }

    /// Writes the archive through a temp file, only when its text differs from the file's.
    pub fn save(&self, char_dir: &Path) -> io::Result<bool> {
        let path = archive_path(char_dir);
        write_if_changed(&path, self.text().as_bytes())
    }

    fn payload_text(&self, name: &str) -> Option<String> {
        let mut out = format!("{PAYLOAD_MAGIC}\t1\t{name}\n");
        let mut any = false;
        for rec in self.records.values() {
            let Some(line) = yc_line(rec) else { continue };
            out.push_str(&line);
            out.push('\n');
            any = true;
        }
        any.then_some(out)
    }
}

fn write_if_changed(path: &Path, bytes: &[u8]) -> io::Result<bool> {
    if fs::read(path).is_ok_and(|old| old == bytes) {
        return Ok(false);
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(true)
}

/// Feeds the records an addon file just showed into a character's archive. Returns how many were
/// added or replaced. An archive that does not read stays as it is, and `rebuild` reports it.
pub fn feed(char_dir: &Path, read: Vec<Record>) -> Result<usize, String> {
    if read.is_empty() {
        return Ok(0);
    }
    let Ok(mut archive) = Archive::load(char_dir) else {
        return Ok(0);
    };
    let changed = archive.merge(read);
    if changed > 0 {
        archive.save(char_dir).map_err(|e| e.to_string())?;
    }
    Ok(changed)
}

/// Feeds records of account scope into the archive of every exported character of one game account
/// folder, so each carries the same text. The once-a-day and only-when-changed rule for a `life`
/// total is applied to what the characters' archives hold together, and a character that lacks an
/// earlier record of the account takes it from the others. Returns how many records were added or
/// replaced across the archives. An archive that does not read stays as it is.
pub fn feed_account(char_dirs: &[PathBuf], read: Vec<Record>) -> Result<usize, String> {
    if read.is_empty() {
        return Ok(0);
    }
    let mut archives: Vec<(&PathBuf, Archive)> = char_dirs
        .iter()
        .filter_map(|dir| Some((dir, Archive::load(dir).ok()?)))
        .collect();
    let mut shared = Archive::default();
    for (_, archive) in &archives {
        for rec in archive.records.values().filter(|r| r.scope == ACCOUNT) {
            shared.records.insert(rec.key(), rec.clone());
        }
    }
    shared.merge(read);
    let mut changed = 0;
    for (dir, archive) in &mut archives {
        let before = changed;
        for rec in shared.records.values() {
            if archive.records.get(&rec.key()) != Some(rec) {
                archive.records.insert(rec.key(), rec.clone());
                changed += 1;
            }
        }
        if changed > before {
            archive.save(dir).map_err(|e| e.to_string())?;
        }
    }
    Ok(changed)
}

/// Rebuilds a character's `counts/payloads/counts.scl` from its archive, or removes it when the
/// archive maps to no line. Returns whether the file changed.
pub fn rebuild(char_dir: &Path, name: &str) -> Result<bool, String> {
    if !archive_path(char_dir).is_file() {
        return Ok(false);
    }
    let archive = Archive::load(char_dir)?;
    let path = payload_path(char_dir);
    let Some(text) = archive.payload_text(name) else {
        return match fs::remove_file(&path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.to_string()),
        };
    };
    let bytes = zstd::stream::encode_all(text.as_bytes(), 19).map_err(|e| e.to_string())?;
    write_if_changed(&path, &bytes).map_err(|e| e.to_string())
}

fn field(rec: &Record, name: &str) -> Option<i64> {
    rec.fields.get(name).copied()
}

fn scarlet_amounts(rec: &Record) -> Vec<(&'static str, i64)> {
    if rec.kind != FISHING {
        return Vec::new();
    }
    match (rec.origin.as_str(), rec.window.as_str()) {
        (FISHING_LOG, SESSION) => fishing_log_amounts(rec),
        (FOREVER_FISHING, SESSION) => forever_fishing_amounts(rec),
        (MASTERCRAFT, LIFE) => mastercraft_amounts(rec),
        _ => Vec::new(),
    }
}

fn fishing_log_amounts(rec: &Record) -> Vec<(&'static str, i64)> {
    let mut out = Vec::new();
    for (metric, source) in [
        ("casts", "outcomes.casts"),
        ("landed", "outcomes.landed"),
        ("unknown", "outcomes.unknown"),
    ] {
        if let Some(n) = field(rec, source) {
            out.push((metric, n));
        }
    }
    let count = field(rec, "count");
    let junk = field(rec, "spots.junk");
    if count.is_some() || junk.is_some() {
        out.push(("items", count.unwrap_or(0) + junk.unwrap_or(0)));
    }
    out
}

fn forever_fishing_amounts(rec: &Record) -> Vec<(&'static str, i64)> {
    [
        ("casts", "casts"),
        ("landed", "successfulCasts"),
        ("items", "catches"),
    ]
    .into_iter()
    .filter_map(|(metric, source)| Some((metric, field(rec, source)?)))
    .collect()
}

fn mastercraft_amounts(rec: &Record) -> Vec<(&'static str, i64)> {
    [
        ("casts", "casts"),
        ("landed", "catches"),
        ("items", "gained"),
    ]
    .into_iter()
    .filter_map(|(metric, source)| Some((metric, field(rec, source)?)))
    .collect()
}

fn yc_line(rec: &Record) -> Option<String> {
    let amounts = scarlet_amounts(rec);
    if amounts.is_empty() || (rec.window != LIFE && rec.from.is_none()) {
        return None;
    }
    let amounts: Vec<String> = amounts.iter().map(|(m, n)| format!("{m}={n}")).collect();
    Some(
        [
            (rec.to * 1000).to_string(),
            "YC".to_string(),
            rec.origin.clone(),
            rec.store.clone(),
            rec.scope.clone(),
            rec.window.clone(),
            rec.kind.clone(),
            rec.from.map(|f| f.to_string()).unwrap_or_default(),
            rec.to.to_string(),
            rec.dims.clone(),
            amounts.join(";"),
        ]
        .join("\t"),
    )
}

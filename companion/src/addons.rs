//! The addon stream: lines Scarlet records from other addons, kept in their
//! own SavedVariables table (`ScarletAddonsDB`) and written under
//! `<out>/<Name-Surname>/addons/`, apart from the main payloads. The line
//! format is `<dt>\t<integration key>\t<code>\t<fields>`; see docs/ADDONS.md
//! in the addon repository.

use crate::export::{chunk, decode};
use crate::lua::Lua;
use chrono::{Local, TimeZone};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;

/// One addon line. `cs` is the absolute time in epoch centiseconds.
pub struct AddonEv {
    pub cs: i64,
    pub key: String,
    pub code: String,
    pub fields: Vec<String>,
}

/// Decode chunk text. `base` is the chunk's time base in epoch seconds.
pub fn parse_chunk_text(text: &str, base: i64) -> Vec<AddonEv> {
    let mut cum: i64 = 0;
    let mut out = Vec::new();
    for line in text.split('\n') {
        if line.is_empty() {
            continue;
        }
        let mut it = line.split('\t');
        let dt = it.next().and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
        cum += dt;
        let key = it.next().unwrap_or("").to_string();
        let code = it.next().unwrap_or("").to_string();
        if key.is_empty() || code.is_empty() {
            continue;
        }
        out.push(AddonEv {
            cs: base * 100 + cum,
            key,
            code,
            fields: it.map(str::to_string).collect(),
        });
    }
    out
}

/// The integration's display name; an unknown key shows as itself.
pub fn label(key: &str) -> &str {
    match key {
        "gm" => "GatherMate2",
        "fl" => "The Fishing Log",
        "ff" => "Forever Fishing",
        "ps" => "Polestar",
        "pl" => "Profession Shopping List",
        "at" => "Attune",
        "al" => "AutoLayer",
        other => other,
    }
}

fn f(fields: &[String], i: usize) -> &str {
    fields.get(i).map_or("", String::as_str)
}

/// Map coordinates are written times 10000.
fn pct(s: &str) -> String {
    s.parse::<f64>()
        .map_or_else(|_| s.to_string(), |v| format!("{:.1}", v / 100.0))
}

fn flags(s: &str) -> String {
    let mut out = Vec::new();
    for c in s.chars() {
        out.push(match c {
            's' => "specimen",
            'j' => "junk",
            'f' => "frenzy",
            'b' => "personal best",
            _ => continue,
        });
    }
    out.join(", ")
}

/// A readable description of one line. Unknown codes show their fields.
pub fn details(key: &str, code: &str, p: &[String]) -> String {
    match (key, code) {
        ("gm", "N") => format!(
            "{} ({}) map {} at {}, {}",
            f(p, 4),
            f(p, 3),
            f(p, 0),
            pct(f(p, 1)),
            pct(f(p, 2))
        ),
        ("fl", "C") => {
            let mut s = format!("item {} {} lb", f(p, 0), f(p, 1));
            let fl = flags(f(p, 2));
            if !fl.is_empty() {
                s.push_str(&format!(" ({fl})"));
            }
            s.push_str(&format!(" at {}", f(p, 4)));
            if !f(p, 5).is_empty() {
                s.push_str(&format!(
                    " map {} {}, {}",
                    f(p, 5),
                    pct(f(p, 6)),
                    pct(f(p, 7))
                ));
            }
            s
        }
        ("ff", "C") => format!("item {} x{} at {}", f(p, 0), f(p, 1), f(p, 2)),
        ("ps", "G") => format!("guide {}", f(p, 0)),
        ("ps", "S") => format!("guide {} step {}", f(p, 0), f(p, 1)),
        ("ps", "D") => format!("guide {} completed", f(p, 0)),
        ("ps", "K") => format!("guide {} skipped steps {} to {}", f(p, 0), f(p, 1), f(p, 2)),
        ("ps", "R") => format!("route {} to {}", f(p, 0), f(p, 1)),
        ("pl", "T") => {
            if f(p, 0).is_empty() {
                "tracked recipes cleared".to_string()
            } else {
                format!("recipe {} quantity {}", f(p, 0), f(p, 1))
            }
        }
        ("at", "S") => format!("attunement {} step {} done", f(p, 0), f(p, 1)),
        ("at", "P") => format!("attunement {} {} {}%", f(p, 0), f(p, 2), f(p, 1)),
        ("al", "R") => "layer request sent".to_string(),
        ("al", "J") => format!("layered someone, {} in all", f(p, 0)),
        _ => p.join("  "),
    }
}

pub fn txt_line(ev: &AddonEv) -> Option<String> {
    let ts = Local
        .timestamp_opt(ev.cs / 100, (ev.cs % 100) as u32 * 10_000_000)
        .single()?;
    let stamp = format!(
        "{}.{:02}",
        ts.format("%Y-%m-%d %H:%M:%S"),
        ev.cs.rem_euclid(100)
    );
    let d = details(&ev.key, &ev.code, &ev.fields);
    Some(
        format!("{stamp}  {:<24}  {d}", label(&ev.key))
            .trim_end()
            .to_string(),
    )
}

/// Raw payload line: `<epoch ms>\t<key>\t<code>\t<fields...>`.
pub fn scl_line(ev: &AddonEv) -> String {
    let mut s = format!("{}\t{}\t{}", ev.cs * 10, ev.key, ev.code);
    if !ev.fields.is_empty() {
        s.push('\t');
        s.push_str(&ev.fields.join("\t"));
    }
    s
}

/// What one export run found for one character.
#[derive(Default)]
pub struct AddonRun {
    pub txt: String,
    pub scl: String,
    pub chunks: u64,
    pub events: u64,
    pub first_q: u64,
    pub last_q: u64,
}

/// Collect the chunks of `ScarletAddonsDB` newer than `last` into `runs`
/// and return the highest chunk number read. A chunk that does not decode
/// adds to `bad`.
pub fn collect(db: &Lua, last: u64, runs: &mut BTreeMap<String, AddonRun>, bad: &mut u64) -> u64 {
    let mut newest = last;
    let Some(chunks) = db.get("chunks").and_then(Lua::items) else {
        return newest;
    };
    for item in chunks {
        let Some(ch) = chunk(&item.1) else { continue };
        if ch.q <= last {
            continue;
        }
        let Some(text) = decode(&ch.e, &ch.z) else {
            *bad += 1;
            continue;
        };
        let run = runs.entry(ch.c.clone()).or_insert_with(|| AddonRun {
            first_q: ch.q,
            last_q: ch.q,
            ..Default::default()
        });
        run.chunks += 1;
        run.last_q = run.last_q.max(ch.q);
        for ev in parse_chunk_text(&text, ch.t) {
            if let Some(line) = txt_line(&ev) {
                run.txt.push_str(&line);
                run.txt.push('\n');
            }
            run.scl.push_str(&scl_line(&ev));
            run.scl.push('\n');
            run.events += 1;
        }
        newest = newest.max(ch.q);
    }
    newest
}

/// Write one character's addon lines under `<dir>/addons/`: a readable
/// `addons.txt` appended to, and one zstd payload per run in `payloads/`.
/// Returns the payload size.
pub fn write_run(
    dir: &Path,
    name: &str,
    run: &AddonRun,
) -> Result<usize, Box<dyn std::error::Error>> {
    let base = dir.join("addons");
    fs::create_dir_all(base.join("payloads"))?;
    if !run.txt.is_empty() {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(base.join("addons.txt"))?;
        file.write_all(run.txt.as_bytes())?;
    }
    let payload = zstd::stream::encode_all(
        format!("scarletaddons\t1\t{name}\n{}", run.scl).as_bytes(),
        19,
    )?;
    fs::write(
        base.join("payloads")
            .join(format!("{}-{}.scl", run.first_q, run.last_q)),
        &payload,
    )?;
    Ok(payload.len())
}

/// Every payload of a character's addon stream, in time order.
pub fn read_payloads(char_dir: &Path) -> Vec<AddonEv> {
    let mut files: Vec<_> = fs::read_dir(char_dir.join("addons").join("payloads"))
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("scl"))
        .collect();
    files.sort();
    let mut per_file = Vec::new();
    for p in files {
        let mut out = Vec::new();
        let Ok(raw) = fs::read(&p) else { continue };
        let Ok(mut dec) = zstd::stream::Decoder::new(&raw[..]) else {
            continue;
        };
        let mut text = String::new();
        if std::io::Read::read_to_string(&mut dec, &mut text).is_err() {
            continue;
        }
        for line in text.split('\n').skip(1) {
            let mut it = line.trim_end_matches('\r').splitn(4, '\t');
            let Ok(ms) = it.next().unwrap_or("").parse::<i64>() else {
                continue;
            };
            let key = it.next().unwrap_or("").to_string();
            let code = it.next().unwrap_or("").to_string();
            if key.is_empty() || code.is_empty() {
                continue;
            }
            let fields = match it.next() {
                Some(rest) if !rest.is_empty() => rest.split('\t').map(str::to_string).collect(),
                _ => Vec::new(),
            };
            out.push(AddonEv {
                cs: ms / 10,
                key,
                code,
                fields,
            });
        }
        per_file.push(out);
    }
    let mut out = crate::archive::merge_by(per_file, |e, h| {
        std::hash::Hash::hash(&(e.cs, &e.key, &e.code, &e.fields), h)
    });
    out.sort_by_key(|e| e.cs);
    out
}

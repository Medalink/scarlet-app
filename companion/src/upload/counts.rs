//! Parts of group `counts`, built from `counts/payloads/*.scl` the way `scarlet_part::imports` builds
//! imported history: one part for each UTC day that has `YC` lines, keyed by the day's start in epoch
//! seconds (Scarlet's FORMAT.md and the website's DESIGN.md, "Addon counts").

use crate::counts::{DIR, PAYLOAD_MAGIC};
use scarlet_part::build::{Options, PART_MAGIC, Part, part_id};
use scarlet_part::counts::{CODE, GROUP};
use scarlet_part::payload::{Ev, parse_scl};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

const DAY_MS: i64 = 86_400_000;

/// The lines of a character's counts payloads (`counts/payloads/*.scl`, magic `scarletcounts`), when
/// a file's header names `character`.
pub fn load(char_dir: &Path, character: &str) -> Vec<Ev> {
    let Ok(entries) = fs::read_dir(char_dir.join(DIR).join("payloads")) else {
        return Vec::new();
    };
    let mut files: Vec<_> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("scl"))
        .collect();
    files.sort();
    let mut out = Vec::new();
    for path in files {
        let Ok(bytes) = fs::read(&path) else { continue };
        if let Ok(p) = parse_scl(&bytes, PAYLOAD_MAGIC)
            && p.character == character
        {
            out.extend(p.events);
        }
    }
    out
}

/// One part for each UTC day that has `YC` lines; none while the `counts` group is off.
pub fn build_parts(character: &str, mut events: Vec<Ev>, opts: &Options) -> Vec<Part> {
    if !opts.on.contains(GROUP) {
        return Vec::new();
    }
    events.sort_by_key(|e| e.ms);
    let mut days: BTreeMap<i64, Vec<&Ev>> = BTreeMap::new();
    for ev in &events {
        if ev.code() == CODE {
            days.entry(ev.ms.div_euclid(DAY_MS)).or_default().push(ev);
        }
    }
    days.into_iter()
        .map(|(day, lines)| {
            let session = day * 86_400;
            let mut text = format!("{PART_MAGIC}\t1\t{character}\t{session}\t{GROUP}\n");
            let mut prev = session * 1000;
            for ev in &lines {
                text.push_str(&(ev.ms - prev).to_string());
                text.push('\t');
                text.push_str(&ev.text);
                text.push('\n');
                prev = ev.ms;
            }
            Part {
                id: part_id(&text),
                session,
                group: GROUP.to_string(),
                lines: lines.len(),
                text,
            }
        })
        .collect()
}

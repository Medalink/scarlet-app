//! Reading .scl payload files: zstd text with a one-line header, then one
//! event per line (`<epoch ms>\t<code>\t<fields...>`), the inverse of
//! export's payload writer.

use crate::events;
use std::fs;
use std::io::Read;
use std::path::Path;

/// One event as stored in a payload. `ms` is epoch milliseconds.
pub struct Event {
    pub ms: i64,
    pub code: String,
    pub fields: Vec<String>,
}

pub struct Payload {
    pub character: String,
    pub events: Vec<Event>,
}

/// Decode one .scl file. The header is `scarlet\t1\t<Name-Surname>`.
pub fn read_payload(path: &Path) -> std::io::Result<Payload> {
    let raw = fs::read(path)?;
    let mut decoder = zstd::stream::Decoder::new(&raw[..])?;
    let mut text = String::new();
    decoder.read_to_string(&mut text)?;
    Ok(parse_payload_text(&text))
}

/// Parse decoded payload text. Lines that do not start with a decimal
/// millisecond are skipped, so a truncated tail never panics.
pub fn parse_payload_text(text: &str) -> Payload {
    let mut it = text.split('\n');
    let header = it.next().unwrap_or("");
    let mut parts = header.split('\n').next().unwrap_or("").split('\t');
    let character = parts
        .nth(2)
        .unwrap_or("")
        .trim_end_matches('\r')
        .to_string();

    let mut events = Vec::new();
    for line in it {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let mut f = line.splitn(3, '\t');
        let Ok(ms) = f.next().unwrap_or("").parse::<i64>() else {
            continue;
        };
        let code = f.next().unwrap_or("").to_string();
        let fields = match f.next() {
            Some(rest) if !rest.is_empty() => rest.split('\t').map(str::to_string).collect(),
            _ => Vec::new(),
        };
        events.push(Event { ms, code, fields });
    }
    Payload { character, events }
}

/// Every .scl payload for one character directory (`<out>/<Name-Surname>/payloads`).
pub fn char_payloads(char_dir: &Path) -> std::io::Result<Vec<std::path::PathBuf>> {
    let mut out = Vec::new();
    let dir = char_dir.join("payloads");
    if let Ok(entries) = fs::read_dir(&dir) {
        for e in entries {
            let p = e?.path();
            if p.extension().and_then(|s| s.to_str()) == Some("scl") {
                out.push(p);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// `Name-Surname` from the payload header, without decoding the body. Used to
/// group payloads by character before a full read.
pub fn payload_character(path: &Path) -> Option<String> {
    let raw = fs::read(path).ok()?;
    let mut decoder = zstd::stream::Decoder::new(&raw[..]).ok()?;
    let mut head = [0u8; 256];
    let n = decoder.read(&mut head).unwrap_or(0);
    let head = String::from_utf8_lossy(&head[..n]);
    head.split('\t').nth(2).map(|s| s.trim().to_string())
}

/// Names defined by an event's code, if it is a name line (N, SN, IN).
pub fn name_of(ev: &Event) -> Option<(char, String, String)> {
    let kind = events::name_kind(&ev.code)?;
    let id = ev.fields.first()?;
    let name = ev.fields.get(1)?;
    Some((kind, id.clone(), name.clone()))
}

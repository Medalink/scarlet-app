//! `scarlet upload ...`: drives the upload client without the app, and prints the bytes of
//! every request so it can be measured.

use super::api::Hit;
use super::config::{self, UploadConfig};
use super::keys;
use super::session::{CharReport, Session};
use super::state;
use super::{Error, Result};
use std::path::PathBuf;

pub const USAGE: &str = "usage: scarlet upload <command> [--out <dir>] [--settings <settings.json>]
  status                       the settings, what was last sent and what the server holds
  sync [--character <name>]    upload what is new for the characters that are on
  groups                       the sharing groups and which are on
  remove-group <key>           remove a group from the server and switch it off here
  restore-group <key>          let the server take a group again and switch it on here
  delete-all --yes             remove everything this install uploaded
  link                         print a link code and the page to type it on
  set [--enable|--disable] [--server <url>] [--group <key>=on|off]... [--set-character <name>=on|off]...
  forget-identity              delete this PC's install key from the credential store";

#[derive(Default)]
pub struct Args {
    pub words: Vec<String>,
    pub out: PathBuf,
    pub settings: PathBuf,
    pub character: Option<String>,
    pub yes: bool,
    pub enable: bool,
    pub disable: bool,
    pub server: Option<String>,
    pub groups: Vec<String>,
    pub characters: Vec<String>,
}

pub fn run(a: Args) -> Result<()> {
    let Some(cmd) = a.words.first().cloned() else {
        return Err(Error::local(USAGE));
    };
    let key = a.words.get(1).cloned();
    match cmd.as_str() {
        "status" => status(&a),
        "sync" => sync(&a),
        "groups" => groups(&a),
        "remove-group" => remove_group(&a, key.ok_or_else(|| Error::local(USAGE))?),
        "restore-group" => restore_group(&a, key.ok_or_else(|| Error::local(USAGE))?),
        "delete-all" => delete_all(&a),
        "link" => link(&a),
        "set" => set(&a),
        "forget-identity" => forget(&a),
        _ => Err(Error::local(USAGE)),
    }
}

fn bytes(n: u64) -> String {
    let units = ["bytes", "KB", "MB", "GB"];
    let (mut v, mut i) = (n as f64, 0);
    while v >= 1024.0 && i < units.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} bytes")
    } else {
        format!("{v:.1} {}", units[i])
    }
}

fn print_hit(h: &Hit) {
    println!(
        "  {:<6} {:<34} {}  sent {}+{}  received {}+{}  {} ms",
        h.method, h.path, h.status, h.sent_body, h.sent_head, h.recv_body, h.recv_head, h.ms
    );
}

fn load(a: &Args) -> Result<UploadConfig> {
    config::load(&a.settings)
}

fn open(a: &Args, cfg: &UploadConfig, create: bool) -> Result<Option<Session>> {
    Session::open(cfg, &a.out, keys::system_store().as_ref(), create)
}

fn status(a: &Args) -> Result<()> {
    let cfg = load(a)?;
    println!(
        "uploading is {}; server {}",
        if cfg.enabled { "on" } else { "off" },
        if cfg.server.is_empty() {
            "(none)"
        } else {
            &cfg.server
        }
    );
    let off_keys: Vec<String> = config::all_groups()
        .into_iter()
        .filter(|g| g.switchable && !cfg.group_on(&g.key))
        .map(|g| g.key)
        .collect();
    println!(
        "groups off: {}",
        if off_keys.is_empty() {
            "none".into()
        } else {
            off_keys.join(", ")
        }
    );
    let st = state::load(&a.out);
    for (name, c) in &st.characters {
        println!(
            "{name}: {} parts, {} sessions, last sync {}, sent {} last and {} in all",
            c.acked.len(),
            c.sessions,
            if c.last_sync == 0 {
                "never".into()
            } else {
                format!("at {}", c.last_sync)
            },
            bytes(c.last_sent),
            bytes(c.total_sent)
        );
    }
    for e in st.log.iter().rev().take(10) {
        let what = match (&e.error, e.parts) {
            (Some(err), _) => format!("failed: {err}"),
            (None, 0) => "nothing new".into(),
            (None, p) => format!(
                "sent {p} parts of {} sessions for {} characters",
                e.sessions,
                e.characters.len()
            ),
        };
        let again = if e.repeats > 0 {
            format!(" ({} more times)", e.repeats)
        } else {
            String::new()
        };
        println!("upload at {}: {what}, {}{again}", e.at, bytes(e.sent));
    }
    if cfg.server_problem().is_some() {
        return Ok(());
    }
    let Some(mut s) = open(a, &cfg, false)? else {
        println!("this PC has no install key yet");
        return Ok(());
    };
    let me = s.me()?;
    println!(
        "linked to an account: {}",
        if me.linked { "yes" } else { "no" }
    );
    let server = s.server_status()?;
    if let Some(list) = server.get("characters").and_then(|c| c.as_array()) {
        for (i, c) in list.iter().enumerate() {
            let n = |k: &str| c.get(k).and_then(|v| v.as_u64()).unwrap_or(0);
            println!(
                "server holds character {}: {} parts, {} bytes of text, {} sessions ({} waiting to decode)",
                i + 1,
                n("parts"),
                n("raw_bytes"),
                n("sessions"),
                n("sessions_waiting")
            );
        }
    }
    if let Some(r) = server.get("groups_refused").and_then(|g| g.as_array()) {
        let r: Vec<&str> = r.iter().filter_map(|g| g.as_str()).collect();
        println!(
            "groups removed on the server: {}",
            if r.is_empty() {
                "none".into()
            } else {
                r.join(", ")
            }
        );
    }
    for h in s.hits() {
        print_hit(h);
    }
    Ok(())
}

fn print_report(r: &CharReport, label: &str) {
    let what = if let Some(n) = &r.note {
        n.clone()
    } else if r.matched {
        "nothing new".into()
    } else {
        format!(
            "sent {} parts of {} sessions",
            r.parts_sent, r.sessions_sent
        )
    };
    println!(
        "{label}: {} parts held here; {what}; {} requests, sent {}, received {}, {} ms{}",
        r.parts,
        r.requests,
        bytes(r.sent),
        bytes(r.received),
        r.ms,
        if r.failed { " (failed)" } else { "" }
    );
}

fn sync(a: &Args) -> Result<()> {
    let cfg = load(a)?;
    if !cfg.enabled {
        return Err(Error::local(
            "Uploading is off. Turn it on with: scarlet upload set --enable",
        ));
    }
    let Some(mut s) = open(a, &cfg, true)? else {
        return Err(Error::local("no install key"));
    };
    let reports = s.sync_all(a.character.as_deref());
    for h in s.hits() {
        print_hit(h);
    }
    let (mut parts, mut sessions) = (0, 0);
    for (i, r) in reports.iter().enumerate() {
        print_report(r, &format!("character {}", i + 1));
        parts += r.parts_sent;
        sessions += r.sessions_sent;
        if !r.refused.is_empty() {
            println!(
                "  the server removed {}: switched off here",
                r.refused.join(", ")
            );
        }
    }
    // Every request of the run, registration included.
    let reqs = s.hits().len();
    let sent: u64 = s.hits().iter().map(Hit::sent).sum();
    let got: u64 = s.hits().iter().map(Hit::received).sum();
    println!(
        "all: {reqs} requests, sent {} ({sent} bytes), received {} ({got} bytes), {parts} parts of {sessions} sessions",
        bytes(sent),
        bytes(got)
    );
    let refused: Vec<String> = reports.iter().flat_map(|r| r.refused.clone()).collect();
    if !refused.is_empty() {
        config::update(&a.settings, |c| {
            for g in &refused {
                c.groups.insert(g.clone(), false);
            }
        })?;
    }
    if reports.iter().any(|r| r.failed) {
        return Err(Error::local("some characters were not uploaded"));
    }
    Ok(())
}

fn groups(a: &Args) -> Result<()> {
    let cfg = load(a)?;
    for g in config::all_groups() {
        let state = if !g.switchable {
            "always"
        } else if cfg.group_on(&g.key) {
            "on"
        } else {
            "off"
        };
        println!("{:<12} {:<7} {}", g.key, state, g.label);
    }
    Ok(())
}

fn remove_group(a: &Args, key: String) -> Result<()> {
    let cfg = load(a)?;
    let Some(mut s) = open(a, &cfg, false)? else {
        println!("nothing was uploaded from this PC");
        return Ok(());
    };
    let (parts, rows) = s.remove_group(&key)?;
    config::update(&a.settings, |c| {
        c.groups.insert(key.clone(), false);
    })?;
    println!("{key}: removed {parts} parts and {rows} rows from the server; the group is off here");
    s.hits().iter().for_each(print_hit);
    Ok(())
}

fn restore_group(a: &Args, key: String) -> Result<()> {
    let cfg = load(a)?;
    let Some(mut s) = open(a, &cfg, true)? else {
        return Err(Error::local("no install key"));
    };
    s.restore_group(&key)?;
    config::update(&a.settings, |c| {
        c.groups.insert(key.clone(), true);
    })?;
    println!("{key}: on again; the next sync sends its parts");
    s.hits().iter().for_each(print_hit);
    Ok(())
}

fn delete_all(a: &Args) -> Result<()> {
    if !a.yes {
        return Err(Error::local(
            "This removes every part, every decoded row and the characters from the server. Add --yes to do it.",
        ));
    }
    let cfg = load(a)?;
    let Some(mut s) = open(a, &cfg, false)? else {
        println!("nothing was uploaded from this PC");
        return Ok(());
    };
    let (chars, parts, rows) = s.delete_all()?;
    println!("removed {chars} characters, {parts} parts and {rows} rows from the server");
    s.hits().iter().for_each(print_hit);
    Ok(())
}

fn link(a: &Args) -> Result<()> {
    let cfg = load(a)?;
    let Some(mut s) = open(a, &cfg, true)? else {
        return Err(Error::local("no install key"));
    };
    if s.me()?.linked {
        println!("this install is linked to an account already");
        return Ok(());
    }
    let l = s.link()?;
    println!("code {} (valid until {})", l.code, l.expires_at);
    println!("open {} and type the code there", l.url);
    s.hits().iter().for_each(print_hit);
    Ok(())
}

fn on_off(v: &str) -> Result<(String, bool)> {
    let (k, val) = v
        .rsplit_once('=')
        .ok_or_else(|| Error::local(format!("'{v}' is not <name>=on|off")))?;
    match val {
        "on" => Ok((k.to_string(), true)),
        "off" => Ok((k.to_string(), false)),
        _ => Err(Error::local(format!("'{v}' is not <name>=on|off"))),
    }
}

fn set(a: &Args) -> Result<()> {
    let mut groups = Vec::new();
    for g in &a.groups {
        let (k, on) = on_off(g)?;
        if !config::is_switchable(&k) {
            return Err(Error::local(format!(
                "'{k}' is not a group that can be switched"
            )));
        }
        groups.push((k, on));
    }
    let mut chars = Vec::new();
    for c in &a.characters {
        chars.push(on_off(c)?);
    }
    let cur = load(a)?;
    let server = a.server.clone().unwrap_or_else(|| cur.server.clone());
    if (a.enable || (cur.enabled && !a.disable))
        && let Some(p) = config::server_problem(&server)
    {
        return Err(Error::local(format!("Uploading cannot be on: {p}")));
    }
    let cfg = config::update(&a.settings, |c| {
        if let Some(s) = &a.server {
            c.server = match s.trim() {
                "" => config::DEFAULT_SERVER.to_string(),
                s => s.to_string(),
            };
        }
        if a.enable {
            c.enabled = true;
        }
        if a.disable {
            c.enabled = false;
        }
        for (k, on) in groups {
            c.groups.insert(k, on);
        }
        for (k, on) in chars {
            c.characters.insert(k, on);
        }
    })?;
    println!(
        "uploading is {}; server {}",
        if cfg.enabled { "on" } else { "off" },
        if cfg.server.is_empty() {
            "(none)"
        } else {
            &cfg.server
        }
    );
    Ok(())
}

fn forget(a: &Args) -> Result<()> {
    keys::system_store().delete()?;
    let mut st = state::load(&a.out);
    st.installs.clear();
    st.characters.clear();
    if state::path(&a.out).exists() {
        state::save(&a.out, &st)?;
    }
    println!("the install key is deleted; uploading starts with a new identity");
    Ok(())
}

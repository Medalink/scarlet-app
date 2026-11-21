//! scarlet: companion CLI for the Scarlet WoW addon. Turns SavedVariables
//! chunk records into a readable .txt per character plus a zstd upload
//! payload, per docs/FORMAT.md.

use lexopt::prelude::*;
use scarlet::upload::cli;
use scarlet::{DEFAULT_WOW, export};
use std::error::Error;
use std::path::PathBuf;

fn main() {
    if let Err(e) = run() {
        eprintln!("scarlet: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut words: Vec<String> = Vec::new();
    let mut wow: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut settings: Option<PathBuf> = None;
    let mut up = cli::Args::default();
    let mut ap = lexopt::Parser::from_env();
    while let Some(arg) = ap.next()? {
        match arg {
            Long("wow") => wow = Some(ap.value()?.into()),
            Long("out") => out = Some(ap.value()?.into()),
            Long("settings") => settings = Some(ap.value()?.into()),
            Long("character") => up.character = Some(ap.value()?.string()?),
            Long("server") => up.server = Some(ap.value()?.string()?),
            Long("group") => up.groups.push(ap.value()?.string()?),
            Long("set-character") => up.characters.push(ap.value()?.string()?),
            Long("yes") => up.yes = true,
            Long("enable") => up.enable = true,
            Long("disable") => up.disable = true,
            Value(v) => words.push(v.string()?),
            _ => return Err(arg.unexpected().into()),
        }
    }
    let Some(cmd) = words.first().cloned() else {
        eprintln!("usage: scarlet <export|watch|logs|upload> [--wow <dir>] [--out <dir>]");
        std::process::exit(2);
    };
    let out = out
        .or_else(|| std::env::var("SCARLET_OUT").ok().map(PathBuf::from))
        .unwrap_or_else(default_out);
    if cmd == "upload" {
        up.words = words[1..].to_vec();
        up.out = out;
        up.settings = settings
            .or_else(|| std::env::var("SCARLET_SETTINGS").ok().map(PathBuf::from))
            .unwrap_or_else(default_settings);
        return Ok(cli::run(up)?);
    }
    if words.len() > 1 {
        return Err("unexpected argument".into());
    }
    // Without --wow: the old default when it exists, else a search of the drives.
    let wow = wow.unwrap_or_else(|| {
        let default = PathBuf::from(DEFAULT_WOW);
        if default.is_dir() {
            default
        } else {
            scarlet::gamedir::find_wow().unwrap_or(default)
        }
    });
    match cmd.as_str() {
        "export" => export::export(&wow, &out),
        "watch" => export::watch(&wow, &out),
        "logs" => export::logs(&wow, &out),
        other => {
            Err(format!("unknown command '{other}' (use export, watch, logs or upload)").into())
        }
    }
}

/// `<exe dir>\data`.
fn default_out() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|d| d.join("data")))
        .unwrap_or_else(|| PathBuf::from("data"))
}

/// `<exe dir>\settings.json`, the app's portable settings file.
fn default_settings() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|d| d.join("settings.json")))
        .unwrap_or_else(|| PathBuf::from("settings.json"))
}

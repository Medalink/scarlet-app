//! scarlet: companion CLI for the Scarlet WoW addon. Turns SavedVariables
//! chunk records into a readable .txt per character plus a zstd upload
//! payload, per docs/FORMAT.md.

use lexopt::prelude::*;
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
    let mut command: Option<String> = None;
    let mut wow: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut ap = lexopt::Parser::from_env();
    while let Some(arg) = ap.next()? {
        match arg {
            Long("wow") => wow = Some(ap.value()?.into()),
            Long("out") => out = Some(ap.value()?.into()),
            Value(v) => {
                if command.is_none() {
                    command = Some(v.string()?);
                } else {
                    return Err("unexpected argument".into());
                }
            }
            _ => return Err(arg.unexpected().into()),
        }
    }
    let Some(cmd) = command else {
        eprintln!("usage: scarlet <export|watch|logs> [--wow <dir>] [--out <dir>]");
        std::process::exit(2);
    };
    // Without --wow: the old default when it exists, else a search of the drives.
    let wow = wow.unwrap_or_else(|| {
        let default = PathBuf::from(DEFAULT_WOW);
        if default.is_dir() {
            default
        } else {
            scarlet::gamedir::find_wow().unwrap_or(default)
        }
    });
    let out = out
        .or_else(|| std::env::var("SCARLET_OUT").ok().map(PathBuf::from))
        .unwrap_or_else(default_out);
    match cmd.as_str() {
        "export" => export::export(&wow, &out),
        "watch" => export::watch(&wow, &out),
        "logs" => export::logs(&wow, &out),
        other => Err(format!("unknown command '{other}' (use export, watch or logs)").into()),
    }
}

/// `<exe dir>\data`.
fn default_out() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|d| d.join("data")))
        .unwrap_or_else(|| PathBuf::from("data"))
}

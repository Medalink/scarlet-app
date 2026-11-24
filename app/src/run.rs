//! One WoW run: from the earliest WoW client's start to the moment the last one
//! exited, and everything the store recorded in the sessions that overlap it.

use crate::commands;
use crate::launch;
use crate::recap::{self, SessionRecap};
use crate::session_groups::{Part, SessionMode, groups};
use crate::store::Store;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows_sys::Win32::System::Threading::WaitForMultipleObjects;

/// One WoW run: the earliest client's start to the moment the last client exited, epoch ms.
#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Run {
    pub from: i64,
    pub to: i64,
}

static PENDING: Mutex<Option<Run>> = Mutex::new(None);

const SLACK_MS: i64 = 60_000;

const POLL: Duration = Duration::from_secs(3);

/// The store's sessions (`S`..`ES` spans) that overlap `[from, to]`, as indices,
/// for every character played in the run.
pub fn pick(st: &Store, from: i64, to: i64) -> Vec<usize> {
    st.sessions
        .iter()
        .enumerate()
        .filter(|(_, s)| s.start <= to + SLACK_MS && s.end >= from - SLACK_MS)
        .map(|(i, _)| i)
        .collect()
}

/// True when the store records anything in the run: at least one session overlapping it.
pub fn recorded(st: &Store, from: i64, to: i64) -> bool {
    !pick(st, from, to).is_empty()
}

/// Sets the pending run.
pub fn set_pending(run: Run) {
    *PENDING.lock().unwrap_or_else(|e| e.into_inner()) = Some(run);
}

/// `--wow-run=<from>-<to>` on the window's command line, or None. A malformed
/// value is ignored.
pub fn from_args() -> Option<Run> {
    std::env::args().find_map(|a| {
        let (from, to) = a.strip_prefix("--wow-run=")?.split_once('-')?;
        Some(Run {
            from: from.parse().ok()?,
            to: to.parse().ok()?,
        })
    })
}

fn zones_in<'a>(st: &'a Store, part: &Part) -> &'a [String] {
    st.sessions
        .iter()
        .find(|s| s.ch == part.ch && s.start <= part.start && s.end >= part.end)
        .map_or(&[], |s| s.zones.as_slice())
}

#[derive(Serialize)]
pub struct RunSession {
    char_name: String,
    start: i64,
    end: i64,
    duration: i64,
    zones: Vec<String>,
    title: String,
}

#[derive(Serialize)]
pub struct RunSummary {
    from: i64,
    to: i64,
    sessions: Vec<RunSession>,
    played: i64,
    chars: Vec<String>,
    recaps: Vec<SessionRecap>,
    /// How each recap's character did on its rates against its WoW runs, one list for each recap.
    paces: Vec<Vec<scarlet::goals::Pace>>,
}

/// The summary of the run `[from, to]`, computed from the current store so a view
/// re-rendering on `store-changed` picks up a late export. A run covers every
/// character, one recap each. With `group` the recaps and rows hold only that session's parts, for
/// `ch` alone when given; without it a run is never clipped to the process times, since a run holds several sessions.
pub fn summary(
    st: &Store,
    from: i64,
    to: i64,
    ch: Option<u32>,
    group: Option<&[Part]>,
) -> RunSummary {
    let mut rows: Vec<(u32, i64, i64, &[String])> = if let Some(parts) = group {
        parts
            .iter()
            .filter(|p| ch.is_none_or(|c| p.ch == c))
            .map(|p| (p.ch, p.start, p.end, zones_in(st, p)))
            .collect()
    } else {
        pick(st, from, to)
            .into_iter()
            .map(|i| {
                let s = &st.sessions[i];
                (s.ch, s.start, s.end, s.zones.as_slice())
            })
            .collect()
    };
    rows.sort_by_key(|r| (r.1, r.2, r.0));
    rows.dedup_by_key(|r| (r.0, r.1, r.2));
    let mut chars: Vec<u32> = Vec::new();
    for r in &rows {
        if !chars.contains(&r.0) {
            chars.push(r.0);
        }
    }
    let name_of = |c: u32| st.chars.get(c as usize).cloned().unwrap_or_default();
    let sessions = rows
        .iter()
        .map(|&(c, start, end, zones)| RunSession {
            char_name: name_of(c),
            start,
            end,
            duration: (end - start).max(0) / 1000,
            zones: zones.to_vec(),
            title: recap::title_of(st, c, (start, end)).text,
        })
        .collect();
    let all: Vec<(i64, i64)> = rows.iter().map(|r| (r.1, r.2)).collect();
    let played = scarlet::recap::union_spans(&all)
        .iter()
        .map(|s| (s.1 - s.0) / 1000)
        .sum();
    let (recaps, paces) = chars
        .iter()
        .filter_map(|&c| {
            let mine: Vec<(i64, i64)> = rows
                .iter()
                .filter(|r| r.0 == c)
                .map(|r| (r.1, r.2))
                .collect();
            Some((
                recap::build_spans(st, c, &mine)?,
                recap::paces(st, c, &mine),
            ))
        })
        .unzip();
    RunSummary {
        from,
        to,
        sessions,
        played,
        chars: chars.into_iter().map(name_of).collect(),
        recaps,
        paces,
    }
}

/// The summary of a run, computed on demand. `ch` limits it to one character. `clip` cuts it to the session of the
/// chosen mode that starts at `from`; when the chosen mode has no session starting there, such as after the mode
/// changed, the summary is empty and never the whole run. `wow_run` cuts it to the WoW run of `ch` that is exactly
/// `[from, to]`, whatever the chosen mode, when there is one: the span a record of `scarlet::goals` names.
#[tauri::command]
pub async fn run_summary(
    app: AppHandle,
    from: i64,
    to: i64,
    ch: Option<String>,
    clip: Option<bool>,
    wow_run: Option<bool>,
) -> RunSummary {
    commands::blocking(move || {
        let (mode, marks) = {
            let s = commands::state(&app)
                .settings
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            (s.session_mode, s.session_marks.clone())
        };
        commands::with_store(&app, |st| {
            let c = ch.and_then(|n| st.char_index(&n));
            let own_run = wow_run
                .unwrap_or(false)
                .then(|| {
                    c.and_then(|c| {
                        groups(st, SessionMode::Auto, &[], Some(c), 0)
                            .into_iter()
                            .find(|g| g.from == from && g.to == to)
                            .map(|g| g.parts)
                    })
                })
                .flatten();
            let clipped = own_run.or_else(|| {
                clip.unwrap_or(false).then(|| {
                    groups(st, mode, &marks, c, now_ms())
                        .into_iter()
                        .find(|g| g.from == from)
                        .map_or_else(Vec::new, |g| g.parts)
                })
            });
            summary(st, from, to, c, clipped.as_deref())
        })
    })
    .await
}

/// Returns and clears the pending run.
#[tauri::command(async)]
pub fn take_wow_run() -> Option<Run> {
    PENDING.lock().unwrap_or_else(|e| e.into_inner()).take()
}

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

fn export_new(app: &AppHandle) {
    let (wows, out) = {
        let s = commands::state(app);
        let st = s.settings.lock().unwrap_or_else(|e| e.into_inner());
        (st.folders(), st.data.clone())
    };
    if !wows.is_empty() && scarlet::export::has_new_all(&wows, &out) {
        let _ = crate::watch::export_and_reload(app, &wows, &out);
    }
}

fn run_ended(app: &AppHandle, from: i64) {
    let run = Run { from, to: now_ms() };
    export_new(app);
    if !commands::with_store(app, |st| recorded(st, run.from, run.to)) {
        commands::reload(app);
        if !commands::with_store(app, |st| recorded(st, run.from, run.to)) {
            return;
        }
    }
    set_pending(run);
    let _ = app.emit("wow-run", ());
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// The window's watcher: scans for WoW Forever clients every 3 s (waiting on their
/// handles is the sleep) and keeps the earliest one's creation time as the run's
/// start. A window started with a run first exports what that run left unexported. The run ends only when a scan finds no client at all, so a client started
/// meanwhile keeps it going and a scan that could not read a process path changes
/// nothing. A scan of the e2e folder finds no client, since it holds no Wow.exe.
pub fn watch_window(app: AppHandle) {
    if PENDING.lock().unwrap_or_else(|e| e.into_inner()).is_some() {
        export_new(&app);
    }
    let mut from: Option<i64> = None;
    let mut clients: Vec<HANDLE> = Vec::new();
    loop {
        if !clients.is_empty() {
            let r = unsafe {
                WaitForMultipleObjects(
                    clients.len() as u32,
                    clients.as_ptr(),
                    0,
                    POLL.as_millis() as u32,
                )
            };
            if r == WAIT_TIMEOUT {
                continue;
            }
            let i = r.wrapping_sub(WAIT_OBJECT_0) as usize;
            if i < clients.len() {
                unsafe { CloseHandle(clients.swap_remove(i)) };
            } else {
                for c in clients.drain(..) {
                    unsafe { CloseHandle(c) };
                }
                std::thread::sleep(POLL);
            }
            if !clients.is_empty() {
                continue;
            }
        }
        let targets = launch::exes(
            &commands::state(&app)
                .settings
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .folders(),
        );
        match launch::find(&targets) {
            launch::Seen::Running(found) => {
                from = launch::earliest_start(&found, from);
                clients = found;
                continue;
            }
            launch::Seen::Stopped => {
                if let Some(f) = from.take() {
                    run_ended(&app, f);
                }
            }
            launch::Seen::Unknown => {}
        }
        std::thread::sleep(POLL);
    }
}

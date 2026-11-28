//! Tauri commands: paged and aggregated reads over the store, plus the
//! compress actions. The whole event set never crosses to the UI.

use crate::agg::Agg;
use crate::ops::SessionPrefs;
use crate::settings::{self, Settings};
use crate::store::{Row, Session, Store};
use chrono::{Datelike, Local, TimeZone, Timelike};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::Ordering;
use std::sync::{Mutex, OnceLock, RwLock};
use tauri::{AppHandle, Emitter};

pub struct State {
    pub store: RwLock<Option<Store>>,
    pub settings: Mutex<Settings>,
}

static CELL: OnceLock<State> = OnceLock::new();

/// Loads the settings and the store the first time it is called; later calls (and a call
/// that arrives while the first is still loading) get the same value.
pub fn preload() -> &'static State {
    CELL.get_or_init(|| {
        let settings = settings::load();
        let store = Store::load(&settings.data.clone());
        State {
            store: RwLock::new(Some(store)),
            settings: Mutex::new(settings),
        }
    })
}

pub fn state(_app: &AppHandle) -> &'static State {
    preload()
}

pub fn with_store<T>(app: &AppHandle, f: impl FnOnce(&Store) -> T) -> T {
    let s = state(app);
    let guard = s.store.read().unwrap();
    f(guard.as_ref().expect("store is always loaded"))
}

/// Run a blocking read or computation on Tokio's blocking pool so async workers are never starved.
pub async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .expect("blocking task panicked")
}

/// Replace the store after an export or a settings change, and tell the UI.
pub fn reload(app: &AppHandle) {
    let s = state(app);
    let dir = s.settings.lock().unwrap().data.clone();
    let store = Store::load(&dir);
    *s.store.write().unwrap() = Some(store);
    let _ = app.emit("store-changed", ());
}

#[derive(Serialize)]
pub struct StoreState {
    chars: Vec<String>,
    events: u64,
    lines: u64,
    sessions: u64,
    first_ms: i64,
    last_ms: i64,
    codes: Vec<(String, u64)>,
    maps: Vec<MapInfo>,
    data_dir: String,
    watch: bool,
}

#[derive(Serialize, Clone)]
pub struct MapInfo {
    id: u32,
    name: String,
    points: u64,
    /// Seconds on the map: the gaps between its recorded positions, ignoring gaps over two minutes.
    seconds: u64,
}

#[tauri::command]
pub async fn app_state(app: AppHandle) -> StoreState {
    blocking(move || {
        let watch = crate::watch::WATCH_ON.load(Ordering::Relaxed);
        with_store(&app, |st| StoreState {
            chars: st.chars.clone(),
            events: st.events.len() as u64,
            lines: st.line_count as u64,
            sessions: st.sessions.len() as u64,
            first_ms: st.first_ms,
            last_ms: st.last_ms,
            codes: st
                .code_counts
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect(),
            maps: maps_of(st, None),
            data_dir: st.data_dir.to_string_lossy().to_string(),
            watch,
        })
    })
    .await
}

/// Maps visited by one character (or all), with point counts.
#[tauri::command]
pub async fn map_list(app: AppHandle, ch: Option<String>) -> Vec<MapInfo> {
    blocking(move || {
        with_store(&app, |st| {
            let c = ch.and_then(|n| st.char_index(&n));
            maps_of(st, c)
        })
    })
    .await
}

/// Map art lives in `maps` next to the data folder, as `<uiMapID>.webp`.
fn maps_dir(st: &Store) -> std::path::PathBuf {
    st.data_dir.parent().unwrap_or(&st.data_dir).join("maps")
}

const MAP_GAP_MS: i64 = 120_000;

fn maps_of(st: &Store, ch: Option<u32>) -> Vec<MapInfo> {
    let mut counts: BTreeMap<u32, u64> = BTreeMap::new();
    for pl in st.polylines.iter().filter(|pl| !pl.dash) {
        let owned = pl.session == u32::MAX
            || ch.is_none_or(|cc| {
                st.sessions
                    .get(pl.session as usize)
                    .is_some_and(|s| s.ch == cc)
            });
        if !owned {
            continue;
        }
        *counts.entry(pl.map).or_default() += pl.pts.len() as u64;
    }
    let mut seconds: BTreeMap<u32, u64> = BTreeMap::new();
    let mut last: HashMap<u32, (i64, u32)> = HashMap::new();
    for r in &st.paths {
        if ch.is_some_and(|cc| r.ch != cc) {
            continue;
        }
        let Some(map) = r.fields.first().and_then(|m| m.parse::<u32>().ok()) else {
            continue;
        };
        if let Some(&(at, was)) = last.get(&r.ch)
            && was == map
            && (0..=MAP_GAP_MS).contains(&(r.ms - at))
        {
            *seconds.entry(map).or_default() += ((r.ms - at) / 1000) as u64;
        }
        last.insert(r.ch, (r.ms, map));
    }
    let mut maps: Vec<MapInfo> = counts
        .into_iter()
        .map(|(id, points)| MapInfo {
            id,
            seconds: seconds.get(&id).copied().unwrap_or(0),
            // The client's own map name, then the zone seen while on the map.
            name: st
                .map_index
                .get(&id)
                .map(|(name, _)| name.clone())
                .or_else(|| {
                    crate::openmap::maps()
                        .iter()
                        .find(|m| m.id == id)
                        .map(|m| m.name.clone())
                })
                .or_else(|| st.zones.get(&id).cloned())
                .unwrap_or_else(|| format!("Map {id}")),
            points,
        })
        .collect();
    maps.sort_by(|a, b| {
        b.seconds
            .cmp(&a.seconds)
            .then(b.points.cmp(&a.points))
            .then(a.id.cmp(&b.id))
    });
    maps
}

/// The art for one map as raw bytes, empty when there is none.
#[tauri::command]
pub async fn map_image(app: AppHandle, map: u32) -> tauri::ipc::Response {
    let bytes = blocking(move || {
        let path = with_store(&app, |st| {
            st.map_index.get(&map).and_then(|(_, file)| {
                let name = std::path::Path::new(file);
                if name.components().count() != 1 || name.file_name().is_none() {
                    return None;
                }
                Some(maps_dir(st).join(name))
            })
        })?;
        if std::fs::metadata(&path).ok()?.len() > 16 * 1024 * 1024 {
            return None;
        }
        std::fs::read(path).ok()
    })
    .await;
    tauri::ipc::Response::new(bytes.unwrap_or_default())
}

#[derive(Serialize)]
pub struct TimeSplit {
    label: &'static str,
    seconds: i64,
}

#[derive(Serialize)]
pub struct Section {
    title: &'static str,
    cards: Vec<(&'static str, String)>,
}

#[derive(Serialize)]
pub struct Stats {
    sections: Vec<Section>,
    total_ms: i64,
    time: Vec<TimeSplit>,
    /// Pairs like grouped vs solo, each summing to the recorded time.
    splits: Vec<(TimeSplit, TimeSplit)>,
}

/// Cards whose value is zero are left out, so a section shows only what
/// happened. A section with no cards is dropped.
fn section(title: &'static str, cards: Vec<(&'static str, i64, String)>) -> Option<Section> {
    let cards: Vec<_> = cards
        .into_iter()
        .filter(|(_, n, _)| *n != 0)
        .map(|(l, _, v)| (l, v))
        .collect();
    (!cards.is_empty()).then_some(Section { title, cards })
}

fn n(v: impl Into<i64> + Copy) -> (i64, String) {
    let v: i64 = v.into();
    (v, big(v))
}

/// 1234567 -> "1.23M", 45210 -> "45.2k".
fn big(v: i64) -> String {
    let f = v as f64;
    match v.abs() {
        0..=9_999 => v.to_string(),
        10_000..=999_999 => format!("{:.1}k", f / 1e3),
        _ => format!("{:.2}M", f / 1e6),
    }
}

#[tauri::command]
pub async fn stats(app: AppHandle, ch: Option<String>) -> Stats {
    blocking(move || stats_sync(app, ch)).await
}

fn stats_sync(app: AppHandle, ch: Option<String>) -> Stats {
    with_store(&app, |st| {
        let c = ch.and_then(|n| st.char_index(&n));
        build_stats(
            st,
            |r| c.is_none_or(|cc| r.ch == cc),
            |s| c.is_none_or(|cc| s.ch == cc),
        )
    })
}

/// The Stats the Stats view and a run summary share: over the events `event`
/// picks, with the recorded time of the sessions `session` picks.
pub fn build_stats(
    st: &Store,
    event: impl Fn(&Row) -> bool,
    session: impl Fn(&Session) -> bool,
) -> Stats {
    let mut a = Agg::new();
    for r in st.events.iter().filter(|r| event(r)) {
        a.apply(r.ch, &r.code, &r.fields, r.ms);
    }
    // Time in game: the sum of the sessions (S to ES, or to the last line of
    // one that ended without ES), never the time between them.
    let total = st
        .sessions
        .iter()
        .filter(|s| session(s))
        .map(|s| (s.end - s.start).max(0))
        .sum::<i64>()
        / 1000;
    let still = (total - a.moving_s() - a.taxi_s - a.afk_s).max(0);
    let mut time = vec![
        ("Running", a.foot_s),
        ("Walking", a.walk_s),
        ("Mounted", a.mount_s),
        ("Swimming", a.swim_s),
        ("Flying", a.fly_s),
        ("On flights", a.taxi_s),
        ("AFK", a.afk_s),
        ("Standing still", still),
    ]
    .into_iter()
    .filter(|(_, s)| *s > 0)
    .map(|(label, seconds)| TimeSplit { label, seconds })
    .collect::<Vec<_>>();
    time.sort_by_key(|t| std::cmp::Reverse(t.seconds));
    let card = |label, v: (i64, String)| (label, v.0, v.1);
    let t = |label, s: i64| (label, s, dur(s));
    let sections = [
        section(
            "Character",
            vec![
                t("Time recorded", total),
                card("Levels gained", n(a.levels as i64)),
                card("Experience", n(a.xp)),
                ("Gold earned", a.earned, money(a.earned)),
                ("Gold spent", a.spent, money(a.spent)),
                card("Items looted", n(a.loot as i64)),
                card("Collectibles", n(a.collected as i64)),
                card("Screenshots", n(a.screenshots as i64)),
                card("Cinematics watched", n(a.cinematics_watched as i64)),
                card("Cinematics skipped", n(a.cinematics_skipped as i64)),
            ],
        ),
        section(
            "Combat",
            vec![
                card("Kills", n(a.kills as i64)),
                card("Damage done", n(a.damage)),
                card("Healing done", n(a.healing)),
                card("Damage taken", n(a.taken)),
                card("Best DPS", n(a.best_dps)),
                card("Fights recorded", n(a.fights as i64)),
                card("Deaths", n(a.deaths as i64)),
                t("Time crowd-controlled", a.cc_s),
                card("Duels won", n(a.duels_won as i64)),
                card("Duels lost", n(a.duels_lost as i64)),
                card("Honorable kills", n(a.honor_kills)),
                ("Repair bills", a.repaired, money(a.repaired)),
            ],
        ),
        section(
            "Travel and time",
            vec![
                ("Distance", a.distance(), format_yd(a.distance())),
                card("Steps", n(a.steps() as i64)),
                card("Jumps", n(a.jumps as i64)),
                card("Areas discovered", n(a.discoveries as i64)),
                card("Camp visits", n(a.camps as i64)),
                t("Time at camp", a.camp_s),
                t("Time on flights", a.taxi_s),
            ],
        ),
        section(
            "Quests and world",
            vec![
                card("Quests accepted", n(a.quests as i64)),
                card("Quests completed", n(a.completed as i64)),
                card("Quests abandoned", n(a.abandoned as i64)),
                t("Time casting", a.cast_s),
                card("Spells cast", n(a.casts as i64)),
                card("Buffs gained", n(a.buffs as i64)),
                card("NPCs talked to", n(a.npc_talks as i64)),
                card("Creatures seen", n(a.creatures.len() as i64)),
                card("Vendors catalogued", n(a.vendors.len() as i64)),
                card("Trainers catalogued", n(a.trainers.len() as i64)),
                card("Items identified", n(a.items_known.len() as i64)),
            ],
        ),
        section(
            "Crafting and loot",
            vec![
                card("Crafts started", n(a.crafts_started as i64)),
                card("Items crafted", n(a.crafted)),
                card("First crafts", n(a.first_crafts as i64)),
                card("Concentration spent", n(a.concentration)),
                card(
                    "Loot from creatures",
                    n(a.loot_sources.get("c").copied().unwrap_or(0) as i64),
                ),
                card(
                    "Loot from objects",
                    n(a.loot_sources.get("g").copied().unwrap_or(0) as i64),
                ),
                card(
                    "Loot from bag items",
                    n(a.loot_sources.get("i").copied().unwrap_or(0) as i64),
                ),
                card(
                    "Loot from elsewhere",
                    n(a.loot_sources.get("o").copied().unwrap_or(0) as i64),
                ),
            ],
        ),
        section(
            "People",
            vec![
                card("Players grouped with", n(a.mates.len() as i64)),
                t("Time in a group", a.group_s),
                card("Emotes", n(a.emotes as i64)),
                card("Said lol", n(a.lols as i64)),
                card("Said rip", n(a.rips as i64)),
                card("Chat messages sent", n(a.chat_sent as i64)),
                card("Chat words sent", n(a.chat_words as i64)),
                card("Players inspected", n(a.inspections as i64)),
                card("Players met", n(a.players_met.len() as i64)),
            ],
        ),
    ]
    .into_iter()
    .flatten()
    .collect();
    let pair = |l1, s1: i64, l2| {
        let s1 = s1.min(total);
        (
            TimeSplit {
                label: l1,
                seconds: s1,
            },
            TimeSplit {
                label: l2,
                seconds: total - s1,
            },
        )
    };
    let splits = if total > 0 {
        vec![
            pair("In combat", a.combat_s, "Out of combat"),
            pair("In a group", a.group_s, "Solo"),
            pair("Resting", a.rested_s, "Out in the world"),
            pair("AFK", a.afk_s, "At the keyboard"),
        ]
    } else {
        Vec::new()
    };
    Stats {
        sections,
        total_ms: total * 1000,
        time,
        splits,
    }
}

#[derive(Serialize)]
pub struct CharOverview {
    name: String,
    ov: scarlet::overview::Overview,
    /// Time to the next level, the next tens level and level 60, computed
    /// once in the companion (docs/OVERVIEW.md, "Level estimates").
    eta: scarlet::overview::Eta,
    /// An hour played at the level cap: the page leads with max level data and the
    /// levelling content moves down (docs/OVERVIEW.md, "Max level").
    focus: bool,
}

/// The character overview for the chosen character; with none chosen, the
/// one played most recently. Built from the store's rows through the same
/// model the export writes to Digest.lua.
#[tauri::command]
pub async fn overview(app: AppHandle, ch: Option<String>) -> Option<CharOverview> {
    blocking(move || overview_sync(app, ch)).await
}

fn overview_sync(app: AppHandle, ch: Option<String>) -> Option<CharOverview> {
    with_store(&app, |st| {
        // A named character must exist; "All characters" shows the one played most recently.
        let c = match ch {
            Some(n) => st.char_index(&n)?,
            None => st
                .sessions
                .iter()
                .max_by_key(|s| s.end)
                .map(|s| s.ch)
                .or_else(|| (!st.chars.is_empty()).then_some(0))?,
        };
        let name = st.chars.get(c as usize)?.clone();
        let mut ov = scarlet::overview::build_among(
            &name,
            st.events
                .iter()
                .filter(|r| r.ch == c)
                .map(|r| (r.ms, r.code.as_str(), r.fields.as_slice())),
            &st.chars,
        );
        // Another recorded character is the player's own (a dual box), not a groupmate.
        ov.mates.retain(|m| !st.chars.contains(&m.name));
        ov.imported = scarlet::import::read_view(&st.data_dir.join(&name));
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);
        let eta = ov.eta(now);
        let focus = ov.max_focus();
        Some(CharOverview {
            name,
            ov,
            eta,
            focus,
        })
    })
}

pub struct GoalsView(std::sync::Arc<scarlet::goals::Goals>);

impl Serialize for GoalsView {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.view().serialize(s)
    }
}

/// How far the chosen character is from each ceiling, and its records over its WoW runs (see `scarlet::goals`).
#[tauri::command]
pub async fn goals(app: AppHandle, ch: Option<String>) -> Option<GoalsView> {
    blocking(move || {
        with_store(&app, |st| {
            let c = ch.and_then(|n| st.char_index(&n))?;
            Some(GoalsView(crate::recap::goals(st, c)))
        })
    })
    .await
}

/// `hay` contains `needle_lc` (already lowercase), ignoring ASCII case.
/// Non-ASCII text falls back to a lowercase copy.
fn contains_ci(hay: &str, needle_lc: &str) -> bool {
    if !hay.is_ascii() || !needle_lc.is_ascii() {
        return hay.to_lowercase().contains(needle_lc);
    }
    let (h, n) = (hay.as_bytes(), needle_lc.as_bytes());
    n.is_empty() || h.windows(n.len()).any(|w| w.eq_ignore_ascii_case(n))
}

fn format_yd(yd: i64) -> String {
    if yd < 1760 {
        format!("{yd} yd")
    } else {
        format!("{:.1} mi", yd as f64 / 1760.0)
    }
}

fn dur(secs: i64) -> String {
    let s = secs.max(0);
    let (h, m) = (s / 3600, s / 60 % 60);
    if h > 0 {
        format!("{h}h {m}m")
    } else if m > 0 {
        format!("{m}m {}s", s % 60)
    } else {
        format!("{s}s")
    }
}

fn money(c: i64) -> String {
    if c == 0 {
        return "0c".into();
    }
    if c < 0 {
        return format!("-{}", money(c.saturating_neg()));
    }
    let (g, s, p) = (c / 10_000, c / 100 % 100, c % 100);
    let mut out = String::new();
    if g > 0 {
        out.push_str(&format!("{g}g "));
    }
    if g > 0 || s > 0 {
        out.push_str(&format!("{s}s "));
    }
    out.push_str(&format!("{p}c"));
    out
}

#[derive(Serialize)]
pub struct Heatmap {
    /// 7 x 24 seconds played by local weekday and hour, Monday first.
    week: Vec<u64>,
    max_week: u64,
    /// Seconds played on each local day.
    days: Vec<(String, u64)>,
    max_day: u64,
}

#[tauri::command]
pub async fn heatmap(app: AppHandle, ch: Option<String>) -> Heatmap {
    blocking(move || heatmap_sync(app, ch)).await
}

fn heatmap_sync(app: AppHandle, ch: Option<String>) -> Heatmap {
    with_store(&app, |st| {
        let c = ch.and_then(|n| st.char_index(&n));
        let spans: Vec<(i64, i64)> = st
            .sessions
            .iter()
            .filter(|s| c.is_none_or(|cc| s.ch == cc) && s.end >= s.start)
            .map(|s| (s.start, s.end))
            .collect();
        let mut week = vec![0u64; 7 * 24];
        let mut days: BTreeMap<i32, u64> = BTreeMap::new();
        for (from, to) in scarlet::recap::union_spans(&spans) {
            let mut at = from;
            while at < to {
                let Some(t) = Local.timestamp_millis_opt(at).single() else {
                    break;
                };
                let into_hour = (t.minute() as i64 * 60 + t.second() as i64) * 1000
                    + t.timestamp_subsec_millis() as i64;
                let next = (at + 3_600_000 - into_hour).min(to);
                let secs = ((next - at) / 1000) as u64;
                week[t.weekday().num_days_from_monday() as usize * 24 + t.hour() as usize] += secs;
                *days.entry(t.num_days_from_ce()).or_default() += secs;
                at = next;
            }
        }
        let max_week = week.iter().copied().max().unwrap_or(0);
        let max_day = days.values().copied().max().unwrap_or(0);
        Heatmap {
            week,
            max_week,
            days: days
                .into_iter()
                .filter_map(|(d, n)| {
                    chrono::NaiveDate::from_num_days_from_ce_opt(d)
                        .map(|date| (date.format("%Y-%m-%d").to_string(), n))
                })
                .collect(),
            max_day,
        }
    })
}

#[derive(Serialize)]
pub struct TRow {
    ms: i64,
    code: String,
    word: String,
    who: String,
    text: String,
}

#[derive(Serialize)]
pub struct TimelinePage {
    total: u64,
    rows: Vec<TRow>,
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn timeline(
    app: AppHandle,
    ch: Option<String>,
    q: String,
    codes: Vec<String>,
    from_ms: i64,
    to_ms: i64,
    offset: u64,
    limit: u64,
) -> TimelinePage {
    blocking(move || {
        with_store(&app, |st| {
            let c = ch.and_then(|n| st.char_index(&n));
            let ql = q.to_lowercase();
            let mut rows = Vec::new();
            let mut total = 0u64;
            for l in st.lines() {
                if c.is_some_and(|cc| l.ch != cc) {
                    continue;
                }
                if !codes.is_empty() && !codes.iter().any(|k| k == &l.code) {
                    continue;
                }
                if l.ms < from_ms || l.ms > to_ms {
                    continue;
                }
                if !ql.is_empty() && !contains_ci(&l.text, &ql) {
                    continue;
                }
                total += 1;
                if total > offset && rows.len() < limit as usize {
                    rows.push(TRow {
                        ms: l.ms,
                        code: l.code.clone(),
                        word: l.word.clone(),
                        who: st.chars.get(l.ch as usize).cloned().unwrap_or_default(),
                        text: l.text.clone(),
                    });
                }
            }
            TimelinePage { total, rows }
        })
    })
    .await
}

#[derive(Serialize)]
pub struct SessionRow {
    index: u32,
    char_name: String,
    start: i64,
    end: i64,
    duration: i64,
    zones: Vec<String>,
    kills: u64,
    xp: i64,
    money: i64,
    casts: u64,
}

#[tauri::command(async)]
pub fn sessions(app: AppHandle, ch: Option<String>) -> Vec<SessionRow> {
    with_store(&app, |st| {
        let c = ch.and_then(|n| st.char_index(&n));
        st.sessions
            .iter()
            .enumerate()
            .filter(|(_, s)| c.is_none_or(|cc| s.ch == cc))
            .map(|(i, s)| SessionRow {
                index: i as u32,
                char_name: st.chars.get(s.ch as usize).cloned().unwrap_or_default(),
                start: s.start,
                end: s.end,
                duration: (s.end - s.start).max(0) / 1000,
                zones: s.zones.clone(),
                kills: s.kills,
                xp: s.xp,
                money: s.money,
                casts: s.casts,
            })
            .collect()
    })
}

#[derive(Serialize)]
pub struct SessionSpan {
    from: i64,
    to: i64,
    live: bool,
    played: i64,
    chars: Vec<String>,
}

#[derive(Serialize)]
pub struct SessionCard {
    #[serde(flatten)]
    span: SessionSpan,
    recap: Option<crate::recap::SessionRecap>,
}

#[derive(Serialize)]
pub struct SessionCards {
    prefs: SessionPrefs,
    cards: Vec<SessionCard>,
}

#[derive(Serialize)]
pub struct SessionList {
    prefs: SessionPrefs,
    sessions: Vec<SessionSpan>,
}

fn span_of(st: &Store, g: &crate::session_groups::Group) -> SessionSpan {
    SessionSpan {
        from: g.from,
        to: g.to,
        live: g.live,
        played: g.played(),
        chars: g
            .chars()
            .iter()
            .map(|c| st.chars.get(*c as usize).cloned().unwrap_or_default())
            .collect(),
    }
}

fn session_prefs(app: &AppHandle) -> SessionPrefs {
    let s = state(app)
        .settings
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    SessionPrefs {
        mode: s.session_mode,
        marks: s.session_marks.clone(),
    }
}

/// The sessions of the list, newest first: those with time recorded for their main character, and an open one. The
/// Sessions page and the lists that step between sessions both read this, so Older and Newer never reach a session
/// the page hides.
pub fn listed(
    st: &Store,
    prefs: &SessionPrefs,
    ch: Option<u32>,
    now: i64,
) -> impl Iterator<Item = crate::session_groups::Group> {
    crate::session_groups::groups(st, prefs.mode, &prefs.marks, ch, now)
        .into_iter()
        .filter(|g| g.live || !g.parts.is_empty())
}

fn lite_of(st: &Store, g: &crate::session_groups::Group) -> Option<crate::recap::SessionRecap> {
    let main = *g.chars().first()?;
    crate::recap::lite_spans(st, main, &g.spans_of(main))
}

/// The sessions of the chosen mode, newest first, without their recaps, with the prefs they were cut by. It lists the
/// sessions `session_cards` lists.
#[tauri::command]
pub async fn session_list(app: AppHandle, ch: Option<String>) -> SessionList {
    blocking(move || {
        let prefs = session_prefs(&app);
        with_store(&app, |st| {
            let c = ch.and_then(|n| st.char_index(&n));
            let sessions = listed(st, &prefs, c, crate::run::now_ms())
                .map(|g| span_of(st, &g))
                .collect();
            SessionList { prefs, sessions }
        })
    })
    .await
}

/// One card for each session of the chosen mode, newest first, each with the lite recap of its main character; with
/// `limit`, only the newest that many that have a recap.
#[tauri::command]
pub async fn session_cards(
    app: AppHandle,
    ch: Option<String>,
    limit: Option<usize>,
) -> SessionCards {
    blocking(move || {
        let prefs = session_prefs(&app);
        with_store(&app, |st| {
            let c = ch.and_then(|n| st.char_index(&n));
            let cards = listed(st, &prefs, c, crate::run::now_ms())
                .map(|g| (lite_of(st, &g), g))
                .filter(|(recap, _)| limit.is_none() || recap.is_some())
                .take(limit.unwrap_or(usize::MAX))
                .map(|(recap, g)| SessionCard {
                    span: span_of(st, &g),
                    recap,
                })
                .collect();
            SessionCards { prefs, cards }
        })
    })
    .await
}

#[derive(Serialize)]
pub struct Poly {
    session: u32,
    pts: Vec<(f32, f32)>,
    dash: bool,
}

#[tauri::command(async)]
pub fn map_data(app: AppHandle, map: u32, ch: Option<String>) -> Vec<Poly> {
    with_store(&app, |st| {
        let c = ch.and_then(|n| st.char_index(&n));
        st.polylines
            .iter()
            .filter(|pl| pl.map == map)
            .filter(|pl| {
                pl.session == u32::MAX
                    || c.is_none_or(|cc| {
                        st.sessions
                            .get(pl.session as usize)
                            .is_some_and(|s| s.ch == cc)
                    })
            })
            .map(|pl| Poly {
                session: pl.session,
                pts: downsample(&pl.pts, 2000),
                dash: pl.dash,
            })
            .collect()
    })
}

/// The place names the client's tables give one map, written next to its art as
/// `<uiMapID>.labels.json`; empty when the file is missing or unreadable.
#[tauri::command]
pub async fn map_labels(app: AppHandle, map: u32) -> Vec<serde_json::Value> {
    blocking(move || {
        let path = with_store(&app, |st| maps_dir(st).join(format!("{map}.labels.json")));
        let json = std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
        match json.map(|mut j| j["labels"].take()) {
            Some(serde_json::Value::Array(labels)) => labels,
            _ => Vec::new(),
        }
    })
    .await
}

/// What the Survey look draws on one map besides the walked lines, for one
/// character or all of them. Positions are map coordinates times 10000.
#[tauri::command(async)]
pub fn map_layers(app: AppHandle, map: u32, ch: Option<String>) -> crate::layers::LayersOut {
    with_store(&app, |st| {
        let c = ch.and_then(|n| st.char_index(&n));
        crate::layers::collect(&st.layers, map, c)
    })
}

fn downsample(pts: &[(f32, f32)], cap: usize) -> Vec<(f32, f32)> {
    if pts.len() <= cap {
        return pts.to_vec();
    }
    let step = pts.len() / cap;
    (0..cap).map(|i| pts[i * step]).collect()
}

#[derive(Serialize)]
pub struct Charts {
    xp: Vec<(i64, i64)>,
    money: Vec<(i64, i64)>,
    spells: Vec<(String, u64)>,
    buffs: Vec<(String, i64)>,
    zones: Vec<(String, i64)>,
    items_in: Vec<(String, i64)>,
    items_out: Vec<(String, i64)>,
    people: Vec<(String, i64)>,
    damage_spells: Vec<(String, i64)>,
    healing_spells: Vec<(String, i64)>,
    cast_time: Vec<(String, i64)>,
    kills: Vec<(String, i64)>,
    emotes: Vec<(String, i64)>,
    killers: Vec<(String, i64)>,
}

/// The `n` largest entries, biggest first.
fn top(m: HashMap<String, i64>, n: usize) -> Vec<(String, i64)> {
    let mut v: Vec<_> = m.into_iter().filter(|(_, x)| *x > 0).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v.truncate(n);
    v
}

#[tauri::command]
pub async fn charts(app: AppHandle, ch: Option<String>) -> Charts {
    blocking(move || charts_sync(app, ch)).await
}

fn charts_sync(app: AppHandle, ch: Option<String>) -> Charts {
    with_store(&app, |st| {
        let c = ch.and_then(|n| st.char_index(&n));
        let names = &st.names;
        let spell_name = |id: u64| -> String {
            names
                .get(&scarlet::events::name_key('s', &id.to_string()))
                .cloned()
                .unwrap_or_else(|| format!("Spell {id}"))
        };
        let item_name = |id: u64| -> String {
            names
                .get(&scarlet::events::name_key('i', &id.to_string()))
                .cloned()
                .unwrap_or_else(|| format!("Item {id}"))
        };

        let mut xp: Vec<(i64, i64)> = Vec::new();
        let mut money: Vec<(i64, i64)> = Vec::new();
        let mut xtot = 0i64;
        let mut mtot = 0i64;
        let mut spell_casts: HashMap<u64, u64> = HashMap::new();
        let mut buff_secs: HashMap<u64, i64> = HashMap::new();
        let mut item_in: HashMap<u64, i64> = HashMap::new();
        let mut item_out: HashMap<u64, i64> = HashMap::new();
        let mut people: HashMap<String, i64> = HashMap::new();
        let mut zone_secs: HashMap<String, i64> = HashMap::new();
        let mut cur_zone = String::new();
        let mut prev_ms: Option<i64> = None;
        let mut open_group: HashMap<String, i64> = HashMap::new();
        let mut damage_spells: HashMap<String, i64> = HashMap::new();
        let mut healing_spells: HashMap<String, i64> = HashMap::new();
        let mut cast_time: HashMap<String, i64> = HashMap::new();
        let mut kills: HashMap<String, i64> = HashMap::new();
        let mut emotes: HashMap<String, i64> = HashMap::new();
        let mut killers: HashMap<String, i64> = HashMap::new();
        let mut gates: HashMap<u32, scarlet::events::Deaths> = HashMap::new();
        let field = |r: &crate::store::Row, i: usize| -> i64 {
            r.fields.get(i).and_then(|f| f.parse().ok()).unwrap_or(0)
        };
        let id_of = |r: &crate::store::Row, i: usize| -> u64 {
            r.fields.get(i).and_then(|f| f.parse().ok()).unwrap_or(0)
        };

        for r in &st.events {
            if c.is_some_and(|cc| r.ch != cc) {
                continue;
            }
            // Zone time: event-to-event delta attributed to the current zone.
            if let Some(p) = prev_ms
                && !cur_zone.is_empty()
            {
                let dt = (r.ms - p) / 1000;
                if (0..=3600).contains(&dt) {
                    *zone_secs.entry(cur_zone.clone()).or_default() += dt;
                }
            }
            prev_ms = Some(r.ms);
            match r.code.as_str() {
                "Z" => {
                    cur_zone = r
                        .fields
                        .get(4)
                        .filter(|z| !z.is_empty())
                        .cloned()
                        .unwrap_or_default();
                }
                "X" => {
                    xtot += r.fields.first().and_then(|f| f.parse().ok()).unwrap_or(0);
                    xp.push((r.ms, xtot));
                }
                "M" => {
                    mtot += r.fields.first().and_then(|f| f.parse().ok()).unwrap_or(0);
                    money.push((r.ms, mtot));
                }
                "C" => {
                    if let Some(id) = r.fields.first().and_then(|f| f.parse().ok()) {
                        *spell_casts.entry(id).or_default() += 1;
                        let secs = field(r, 1);
                        if secs > 0 {
                            *cast_time.entry(spell_name(id)).or_default() += secs;
                        }
                    }
                }
                "A" => {
                    if r.fields.first().map(String::as_str) == Some("0") {
                        let id = r.fields.get(1).and_then(|f| f.parse().ok()).unwrap_or(0);
                        *buff_secs.entry(id).or_default() +=
                            r.fields.get(2).and_then(|f| f.parse().ok()).unwrap_or(0);
                    }
                }
                "I" => {
                    if let Some(id) = r.fields.first().and_then(|f| f.parse().ok()) {
                        let d = r.fields.get(1).and_then(|f| f.parse().ok()).unwrap_or(0);
                        if d > 0 {
                            *item_in.entry(id).or_default() += d;
                        } else {
                            *item_out.entry(id).or_default() += -d;
                        }
                    }
                }
                "DS" => {
                    let into = match r.fields.first().map(String::as_str) {
                        Some("d") => &mut damage_spells,
                        Some("h") => &mut healing_spells,
                        _ => continue,
                    };
                    *into.entry(spell_name(id_of(r, 1))).or_default() += field(r, 2);
                }
                "CH" => {
                    *cast_time.entry(spell_name(id_of(r, 0))).or_default() += field(r, 1);
                }
                "K" => {
                    let id = r.fields.first().cloned().unwrap_or_default();
                    let name = names
                        .get(&scarlet::events::name_key('n', &id))
                        .cloned()
                        .unwrap_or_else(|| format!("Creature {id}"));
                    *kills.entry(name).or_default() += 1;
                }
                "EM" => {
                    if let Some(t) = r.fields.first().filter(|t| !t.is_empty()) {
                        *emotes.entry(t.to_lowercase()).or_default() += 1;
                    }
                }
                "D" => {
                    gates.entry(r.ch).or_default().died(r.ms);
                }
                "DR" if r.fields.get(4).is_some_and(|p| !p.is_empty() && p != "1") => {}
                "DR" if r.fields.get(4).is_some_and(|p| p == "1")
                    && !gates.entry(r.ch).or_default().killing_blow(r.ms) => {}
                "DR" => {
                    let who = match (
                        scarlet::events::environmental_killer(&r.fields),
                        r.fields.get(2).filter(|s| !s.is_empty()),
                    ) {
                        (Some(world), _) => world.to_string(),
                        (None, Some(src)) => src.clone(),
                        (None, None) => spell_name(id_of(r, 0)),
                    };
                    *killers.entry(who).or_default() += 1;
                }
                "GM" => {
                    if r.fields.first().map(String::as_str) == Some("1") {
                        if let Some(n) = r.fields.get(1) {
                            open_group.insert(n.clone(), r.ms);
                        }
                    } else if let Some(n) = r.fields.get(1) {
                        let start = open_group.remove(n).unwrap_or(r.ms);
                        *people.entry(n.clone()).or_default() += (r.ms - start) / 1000;
                    }
                }
                _ => {}
            }
        }
        // People still grouped at the end of the data count up to now.
        for (n, start) in open_group {
            *people.entry(n).or_default() += (st.last_ms - start).max(0) / 1000;
        }

        let mut spells: Vec<(String, u64)> = spell_casts
            .into_iter()
            .map(|(id, n)| (spell_name(id), n))
            .collect();
        spells.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        spells.truncate(10);

        let mut buffs: Vec<(String, i64)> = buff_secs
            .into_iter()
            .map(|(id, s)| (spell_name(id), s))
            .collect();
        buffs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        buffs.truncate(10);

        let mut zones: Vec<(String, i64)> = zone_secs.into_iter().collect();
        zones.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        zones.truncate(12);

        let mut items_in: Vec<(String, i64)> = item_in
            .into_iter()
            .map(|(id, n)| (item_name(id), n))
            .collect();
        items_in.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        items_in.truncate(10);
        let mut items_out: Vec<(String, i64)> = item_out
            .into_iter()
            .map(|(id, n)| (item_name(id), n))
            .collect();
        items_out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        items_out.truncate(10);

        let mut people: Vec<(String, i64)> = people.into_iter().collect();
        people.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        Charts {
            xp: bucket(xp, 400),
            money: bucket(money, 400),
            spells,
            buffs,
            zones,
            items_in,
            items_out,
            people,
            damage_spells: top(damage_spells, 10),
            healing_spells: top(healing_spells, 10),
            cast_time: top(cast_time, 10),
            kills: top(kills, 10),
            emotes: top(emotes, 10),
            killers: top(killers, 10),
        }
    })
}

/// Thin the series to about `n` points, keeping the last value per bucket.
fn bucket(series: Vec<(i64, i64)>, n: usize) -> Vec<(i64, i64)> {
    if series.len() <= n {
        return series;
    }
    let step = series.len() / n;
    (0..n)
        .map(|i| {
            let start = i * step;
            let end = if i == n - 1 {
                series.len()
            } else {
                start + step
            };
            series[start..end.max(start + 1).min(series.len())]
                .last()
                .copied()
                .unwrap()
        })
        .collect()
}

#[derive(Serialize)]
pub struct Trend {
    /// SC key or statistic id.
    key: String,
    /// Display name, from WN for statistics.
    name: String,
    series: Vec<(i64, f64)>,
}

#[derive(Serialize)]
pub struct Trends {
    /// Character stats (SC), one series per key.
    stats: Vec<Trend>,
    /// The 20 statistics (WS) that changed most often, biggest first.
    statistics: Vec<Trend>,
}

#[tauri::command]
pub async fn trends(app: AppHandle, ch: Option<String>) -> Trends {
    blocking(move || {
        with_store(&app, |st| {
            let c = ch.and_then(|n| st.char_index(&n));
            let mut sc: BTreeMap<String, Vec<(i64, f64)>> = BTreeMap::new();
            let mut ws_count: HashMap<u64, u64> = HashMap::new();
            let mut ws: HashMap<u64, Vec<(i64, i64)>> = HashMap::new();
            for r in &st.events {
                if c.is_some_and(|cc| r.ch != cc) {
                    continue;
                }
                match r.code.as_str() {
                    "SC" => {
                        if let (Some(k), Some(v)) = (
                            r.fields.get(1),
                            r.fields.get(2).and_then(|f| f.parse::<f64>().ok()),
                        ) {
                            sc.entry(k.clone()).or_default().push((r.ms, v));
                        }
                    }
                    "WS" => {
                        if let Some(id) = r.fields.first().and_then(|f| f.parse::<u64>().ok()) {
                            *ws_count.entry(id).or_default() += 1;
                            if let Some(v) = r
                                .fields
                                .get(1)
                                .and_then(|f| scarlet::tables::stat_number(f))
                            {
                                ws.entry(id).or_default().push((r.ms, v));
                            }
                        }
                    }
                    _ => {}
                }
            }
            let mut top: Vec<(u64, u64)> = ws_count.into_iter().collect();
            top.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            top.truncate(20);
            let statistics = top
                .into_iter()
                .map(|(id, _)| {
                    let name = st
                        .names
                        .get(&scarlet::events::name_key('w', &id.to_string()))
                        .cloned()
                        .unwrap_or_else(|| format!("Statistic {id}"));
                    Trend {
                        key: id.to_string(),
                        name,
                        series: ws
                            .remove(&id)
                            .unwrap_or_default()
                            .into_iter()
                            .map(|(m, v)| (m, v as f64))
                            .collect(),
                    }
                })
                .collect();
            let stats = sc
                .into_iter()
                .map(|(key, series)| Trend {
                    key,
                    name: String::new(),
                    series,
                })
                .collect();
            Trends { stats, statistics }
        })
    })
    .await
}

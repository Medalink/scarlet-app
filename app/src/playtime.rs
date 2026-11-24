//! How much the player plays: their longest day, most played week and longest streak of days, and this week so far
//! against the same days of every earlier week. A day is the group of `SessionMode::Day`, so a record opens the page of
//! the same time that the Sessions list shows in Day mode.

use crate::commands::{blocking, with_store};
use crate::session_groups::{SessionMode, groups};
use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone};
use scarlet::recap::Spread;
use serde::Serialize;
use std::collections::BTreeMap;
use tauri::AppHandle;

const MIN_PEERS: usize = 3;

#[derive(Serialize, Clone)]
pub struct Stretch {
    day: String,
    from: i64,
    to: i64,
    seconds: i64,
}

#[derive(Serialize)]
pub struct Streak {
    days: i64,
    from_day: String,
    to_day: String,
    from: i64,
    to: i64,
}

#[derive(Serialize)]
pub struct WeekPace {
    day: String,
    elapsed_days: i64,
    seconds: i64,
    peers: usize,
    spread: Option<Spread>,
    best: Option<Stretch>,
    is_best: bool,
}

#[derive(Serialize)]
pub struct Playtime {
    longest_day: Option<Stretch>,
    longest_week: Option<Stretch>,
    best_streak: Option<Streak>,
    streak: Option<Streak>,
    week: Option<WeekPace>,
}

struct Day {
    date: NaiveDate,
    from: i64,
    to: i64,
    seconds: i64,
}

fn stretch(day: NaiveDate, from: i64, to: i64, seconds: i64) -> Stretch {
    Stretch {
        day: day.format("%Y-%m-%d").to_string(),
        from,
        to,
        seconds,
    }
}

fn date_of(ms: i64) -> Option<NaiveDate> {
    Local
        .timestamp_millis_opt(ms)
        .single()
        .map(|t| t.date_naive())
}

fn monday(date: NaiveDate) -> NaiveDate {
    date - Duration::days(date.weekday().num_days_from_monday() as i64)
}

fn streak_of(run: &[&Day]) -> Streak {
    let (first, last) = (run[0], run[run.len() - 1]);
    Streak {
        days: run.len() as i64,
        from_day: first.date.format("%Y-%m-%d").to_string(),
        to_day: last.date.format("%Y-%m-%d").to_string(),
        from: first.from,
        to: last.to,
    }
}

fn runs_of(days: &[Day]) -> Vec<Vec<&Day>> {
    let mut out: Vec<Vec<&Day>> = Vec::new();
    for d in days {
        match out.last_mut() {
            Some(run) if run[run.len() - 1].date + Duration::days(1) == d.date => run.push(d),
            _ => out.push(vec![d]),
        }
    }
    out
}

fn compute(days: &[Day], today: NaiveDate) -> Playtime {
    let longest_day = days
        .iter()
        .max_by(|a, b| a.seconds.cmp(&b.seconds).then(b.date.cmp(&a.date)))
        .map(|d| stretch(d.date, d.from, d.to, d.seconds));
    let mut weeks: BTreeMap<NaiveDate, Vec<&Day>> = BTreeMap::new();
    for d in days {
        weeks.entry(monday(d.date)).or_default().push(d);
    }
    let total = |w: &[&Day]| w.iter().map(|d| d.seconds).sum::<i64>();
    let longest_week = weeks
        .iter()
        .max_by(|a, b| total(a.1).cmp(&total(b.1)).then(b.0.cmp(a.0)))
        .map(|(start, w)| {
            stretch(
                *start,
                w.iter().map(|d| d.from).min().unwrap_or(0),
                w.iter().map(|d| d.to).max().unwrap_or(0),
                total(w),
            )
        });
    let runs = runs_of(days);
    let best_streak = runs
        .iter()
        .max_by(|a, b| a.len().cmp(&b.len()).then(b[0].date.cmp(&a[0].date)))
        .map(|r| streak_of(r));
    let streak = runs
        .last()
        .filter(|r| r[r.len() - 1].date + Duration::days(1) >= today)
        .map(|r| streak_of(r));
    let this_week = monday(today);
    let elapsed = today.signed_duration_since(this_week).num_days();
    let so_far = |start: NaiveDate| -> (i64, i64, i64) {
        let inside: Vec<&Day> = weeks
            .get(&start)
            .map(|w| {
                w.iter()
                    .copied()
                    .filter(|d| d.date.signed_duration_since(start).num_days() <= elapsed)
                    .collect()
            })
            .unwrap_or_default();
        (
            total(&inside),
            inside.iter().map(|d| d.from).min().unwrap_or(0),
            inside.iter().map(|d| d.to).max().unwrap_or(0),
        )
    };
    let week = weeks.keys().next().map(|first| {
        let mut peers: Vec<(NaiveDate, i64, i64, i64)> = Vec::new();
        let mut at = *first;
        while at < this_week {
            let (s, from, to) = so_far(at);
            peers.push((at, s, from, to));
            at += Duration::days(7);
        }
        let (now, _, _) = so_far(this_week);
        let enough = peers.len() >= MIN_PEERS;
        WeekPace {
            day: this_week.format("%Y-%m-%d").to_string(),
            elapsed_days: elapsed + 1,
            seconds: now,
            peers: peers.len(),
            spread: enough
                .then(|| Spread::of(peers.iter().map(|p| p.1 as f64).collect(), now as f64)),
            best: peers
                .iter()
                .filter(|p| p.1 > 0)
                .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
                .map(|p| stretch(p.0, p.2, p.3, p.1)),
            is_best: enough && now > 0 && peers.iter().all(|p| now >= p.1),
        }
    });
    Playtime {
        longest_day,
        longest_week,
        best_streak,
        streak,
        week,
    }
}

#[tauri::command]
pub async fn playtime(app: AppHandle, ch: Option<String>) -> Playtime {
    blocking(move || {
        with_store(&app, |st| {
            let c = ch.and_then(|n| st.char_index(&n));
            let mut days: Vec<Day> = groups(st, SessionMode::Day, &[], c, 0)
                .iter()
                .filter_map(|g| {
                    Some(Day {
                        date: date_of(g.from)?,
                        from: g.from,
                        to: g.to,
                        seconds: g.played(),
                    })
                })
                .filter(|d| d.seconds > 0)
                .collect();
            days.sort_by_key(|d| d.date);
            compute(&days, Local::now().date_naive())
        })
    })
    .await
}

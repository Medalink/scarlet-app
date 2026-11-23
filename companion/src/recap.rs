//! The session recap of one character: one shape for a session, or for the sessions of one run that overlap.
//! The rules (title, buckets, stretches, bests) are the ones of CONTRACTS.md and DESIGN.md, "The title".

use crate::agg::Agg;
use crate::events::{Ev, event_parts, name_key};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};

/// One event of one character.
#[derive(Clone, Copy)]
pub struct Rec<'a> {
    pub ms: i64,
    pub code: &'a str,
    pub fields: &'a [String],
}

/// The events of one character, in time order.
pub trait Events {
    fn between(&self, from: i64, to: i64) -> Box<dyn Iterator<Item = Rec<'_>> + '_>;
}

/// What the builder reads about one character: its events, the names the events refer to and the spans of all its
/// sessions (what `own` and `bests` compare against).
pub struct Facts<'a> {
    pub character: &'a str,
    pub events: &'a dyn Events,
    pub names: &'a HashMap<String, String>,
    pub sessions: &'a [(i64, i64)],
}

const BUCKET_SECS: i64 = 300;
const MAX_BUCKETS: i64 = 96;
const MIN_STRETCH_MS: i64 = 30_000;
const HISTORY: usize = 30;
const HISTORY_MIN_MS: i64 = 600_000;
const HISTORY_NEEDED: usize = 5;
const TOP: usize = 8;
const SHEET: f64 = 1000.0;
const SHEET_POINTS: usize = 400;
const LEG_POINTS: usize = 600;
const MAP_ASPECT: f64 = 1.5;
const MILE_YD: f64 = 1760.0;
const GATHER_TITLE: i64 = 20;
const QUEST_TITLE: i64 = 5;
const KILL_TITLE: i64 = 50;
const TRAVEL_MILES: f64 = 3.0;
const TRAVEL_ZONES: usize = 3;

#[derive(Serialize)]
pub struct Title {
    pub rule: &'static str,
    pub text: String,
}

#[derive(Default, Serialize)]
pub struct Xp {
    kills: i64,
    dungeons: i64,
    quests: i64,
    explore: i64,
    other: i64,
}

#[derive(Serialize)]
pub struct Buckets {
    secs: i64,
    xp: Vec<i64>,
    combat: Vec<i64>,
}

#[derive(Serialize)]
pub struct Stretch {
    name: String,
    at: i64,
    secs: i64,
}

#[derive(Serialize)]
pub struct LevelAt {
    level: u32,
    at: i64,
}

#[derive(Serialize)]
pub struct Moment {
    at: i64,
    kind: &'static str,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    feat: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tier: Option<u8>,
}

#[derive(Serialize)]
pub struct Leg {
    map: u32,
    mode: &'static str,
    points: Vec<i64>,
}

#[derive(Serialize)]
pub struct Sheet {
    aspect: f64,
    points: Vec<i64>,
    breaks: Vec<usize>,
    flights: Vec<usize>,
}

#[derive(Serialize)]
pub struct Route {
    #[serde(skip_serializing_if = "Option::is_none")]
    legs: Option<Vec<Leg>>,
    sheet: Sheet,
}

#[derive(Serialize)]
pub struct FeatChange {
    pub key: String,
    pub from: u8,
    pub tier: u8,
    pub at: i64,
    pub value: f64,
}

#[derive(Serialize)]
pub struct FeatNear {
    pub key: String,
    pub tier: u8,
    pub value: f64,
    pub next: f64,
    pub left: f64,
}

#[derive(Default, Serialize)]
pub struct Feats {
    pub earned: Vec<FeatChange>,
    pub near: Vec<FeatNear>,
}

#[derive(Serialize)]
pub struct Spread {
    p10: f64,
    p25: f64,
    p50: f64,
    p75: f64,
    p90: f64,
    you: f64,
}

#[derive(Serialize)]
pub struct Own {
    n: usize,
    metrics: BTreeMap<&'static str, Spread>,
}

#[derive(Serialize)]
pub struct Counted {
    name: String,
    n: i64,
}

#[derive(Serialize)]
pub struct Looted {
    id: i64,
    name: String,
    quality: u8,
    n: i64,
}

#[derive(Serialize)]
pub struct ZoneCount {
    zone: String,
    n: i64,
}

#[derive(Default, Serialize)]
pub struct Top {
    killed: Vec<Counted>,
    looted: Vec<Looted>,
    killers: Vec<Counted>,
    quests_by_zone: Vec<ZoneCount>,
}

#[derive(Serialize)]
pub struct SessionRecap {
    v: u8,
    character: String,
    class: String,
    pub start: i64,
    pub end: i64,
    played: i64,
    level: [u32; 2],
    zone: String,
    pub title: Title,
    xp: Xp,
    runs: i64,
    counters: BTreeMap<&'static str, i64>,
    buckets: Buckets,
    zones: Vec<Stretch>,
    levels: Vec<LevelAt>,
    moments: Vec<Moment>,
    route: Option<Route>,
    pub feats: Feats,
    bests: Vec<&'static str>,
    own: Option<Own>,
    top: Top,
}

struct Raw {
    name: String,
    from: i64,
    to: i64,
    instance: bool,
}

struct Scan<'a> {
    f: &'a Facts<'a>,
    full: bool,
    ch: u32,
    t0: i64,
    last: i64,
    agg: Agg,
    class: String,
    level: [u32; 2],
    zone: Option<String>,
    zone_from: i64,
    zone_seen: bool,
    session_from: i64,
    in_instance: bool,
    instance: Option<String>,
    runs: i64,
    runs_by: HashMap<String, i64>,
    xp: Xp,
    levels: Vec<LevelAt>,
    moments: Vec<Moment>,
    stretches: Vec<Raw>,
    killed: HashMap<String, i64>,
    looted: HashMap<i64, i64>,
    killers: HashMap<String, i64>,
    quest_zones: HashMap<String, i64>,
    combat: Vec<(i64, i64)>,
    xp_at: Vec<(i64, i64)>,
    last_death: Option<(i64, usize)>,
    gathers: i64,
    casts_by_spell: HashMap<String, i64>,
    spec: String,
    bosses: i64,
    flights: i64,
    achievements: i64,
    legs: Vec<RawLeg>,
    leg_open: bool,
}

struct RawLeg {
    map: u32,
    pts: Vec<(f64, f64)>,
    dash: bool,
}

fn field(f: &[String], i: usize) -> &str {
    f.get(i).map_or("", String::as_str)
}

fn number(f: &[String], i: usize) -> i64 {
    field(f, i).parse().unwrap_or(0)
}

fn spans_events<'a, 'b>(
    f: &'a Facts<'a>,
    spans: &'b [(i64, i64)],
) -> impl Iterator<Item = Rec<'a>> + 'b
where
    'a: 'b,
{
    let first = spans.iter().map(|s| s.0).min().unwrap_or(0);
    let last = spans.iter().map(|s| s.1).max().unwrap_or(0);
    f.events
        .between(first, last)
        .filter(move |r| spans.iter().any(|(a, b)| *a <= r.ms && r.ms <= *b))
}

impl<'a> Scan<'a> {
    fn new(f: &'a Facts<'a>, full: bool, t0: i64) -> Self {
        Scan {
            f,
            full,
            ch: 0,
            t0,
            last: t0,
            agg: Agg::new(),
            class: String::new(),
            level: [0, 0],
            zone: None,
            zone_from: t0,
            zone_seen: false,
            session_from: t0,
            in_instance: false,
            instance: None,
            runs: 0,
            runs_by: HashMap::new(),
            xp: Xp::default(),
            levels: vec![],
            moments: vec![],
            stretches: vec![],
            killed: HashMap::new(),
            looted: HashMap::new(),
            killers: HashMap::new(),
            quest_zones: HashMap::new(),
            combat: vec![],
            xp_at: vec![],
            last_death: None,
            gathers: 0,
            casts_by_spell: HashMap::new(),
            spec: String::new(),
            bosses: 0,
            flights: 0,
            achievements: 0,
            legs: vec![],
            leg_open: false,
        }
    }

    fn secs(&self, ms: i64) -> i64 {
        (ms - self.t0).max(0) / 1000
    }

    fn name_of(&self, kind: char, id: &str, fallback: &str) -> String {
        self.f
            .names
            .get(&name_key(kind, id))
            .cloned()
            .unwrap_or_else(|| format!("{fallback} {id}"))
    }

    fn close_zone(&mut self, ms: i64) {
        if let Some(name) = self.zone.take() {
            self.stretches.push(Raw {
                name,
                from: self.zone_from,
                to: ms.max(self.zone_from),
                instance: self.in_instance,
            });
        }
    }

    fn enter_zone(&mut self, r: &Rec) {
        let name = field(r.fields, 4);
        if name.is_empty() {
            return;
        }
        self.close_zone(r.ms);
        self.zone_from = if self.zone_seen {
            r.ms
        } else {
            self.session_from
        };
        self.zone_seen = true;
        self.zone = Some(name.to_string());
        self.in_instance = matches!(field(r.fields, 2), "party" | "raid");
        if self.in_instance {
            let id = field(r.fields, 1);
            if self.instance.as_deref() != Some(id) {
                self.runs += 1;
                *self.runs_by.entry(name.to_string()).or_default() += 1;
                self.instance = Some(id.to_string());
            }
        } else {
            self.instance = None;
        }
    }

    fn death(&mut self, r: &Rec) {
        if self.last_death.is_some_and(|(ms, _)| r.ms - ms < 3000) {
            return;
        }
        self.moments.push(Moment {
            at: self.secs(r.ms),
            kind: "death",
            text: "Died".to_string(),
            feat: None,
            tier: None,
        });
        self.last_death = Some((r.ms, self.moments.len() - 1));
    }

    fn killing_blow(&mut self, r: &Rec) {
        if field(r.fields, 4) != "1" {
            return;
        }
        let env = field(r.fields, 17);
        let source = if field(r.fields, 3) == "1" && !env.is_empty() {
            env
        } else {
            field(r.fields, 2)
        };
        if source.is_empty() {
            return;
        }
        *self.killers.entry(source.to_string()).or_default() += 1;
        if let Some((ms, i)) = self.last_death
            && r.ms - ms < 5000
        {
            self.moments[i].text = format!("Killed by {source}");
        }
    }

    fn experience(&mut self, r: &Rec) {
        let amount = number(r.fields, 0);
        self.xp_at.push((r.ms, amount));
        match field(r.fields, 3) {
            "k" if self.in_instance => self.xp.dungeons += amount,
            "k" => self.xp.kills += amount,
            "q" => self.xp.quests += amount,
            "e" => self.xp.explore += amount,
            _ => self.xp.other += amount,
        }
    }

    fn spec_of(&self, f: &[String]) -> String {
        let named = field(f, 4);
        if !named.is_empty() {
            return named.to_string();
        }
        field(f, 6)
            .split(',')
            .filter_map(|tab| {
                let mut parts = tab.split(':');
                let id = parts.next()?;
                let spent = parts.next()?.parse::<i64>().ok()?;
                Some((spent, id))
            })
            .filter(|(spent, _)| *spent > 0)
            .max_by_key(|(spent, _)| *spent)
            .and_then(|(_, id)| self.f.names.get(&name_key('g', id)).cloned())
            .unwrap_or_default()
    }

    fn position(&mut self, f: &[String]) {
        let (Ok(map), Ok(x), Ok(y)) = (
            field(f, 0).parse::<u32>(),
            field(f, 1).parse::<f64>(),
            field(f, 2).parse::<f64>(),
        ) else {
            return;
        };
        let pt = (x / 100.0, y / 100.0);
        match self.legs.last_mut() {
            Some(l) if self.leg_open && !l.dash && l.map == map => l.pts.push(pt),
            _ => {
                self.legs.push(RawLeg {
                    map,
                    pts: vec![pt],
                    dash: false,
                });
                self.leg_open = true;
            }
        }
    }

    fn teleport(&mut self, f: &[String]) {
        if field(f, 3) != field(f, 6) {
            return;
        }
        let (Ok(map), Ok(fx), Ok(fy), Ok(tx), Ok(ty)) = (
            field(f, 3).parse::<u32>(),
            field(f, 4).parse::<f64>(),
            field(f, 5).parse::<f64>(),
            field(f, 7).parse::<f64>(),
            field(f, 8).parse::<f64>(),
        ) else {
            return;
        };
        self.legs.push(RawLeg {
            map,
            pts: vec![(fx / 100.0, fy / 100.0), (tx / 100.0, ty / 100.0)],
            dash: true,
        });
        self.leg_open = false;
    }

    fn feed(&mut self, r: &Rec) {
        self.last = r.ms;
        if r.code == "P" {
            if self.full {
                self.position(r.fields);
            }
            return;
        }
        let f = r.fields;
        self.agg.apply(self.ch, r.code, f, r.ms);
        match r.code {
            "S" => {
                self.close_zone(r.ms);
                self.leg_open = false;
                self.session_from = r.ms;
                self.zone_seen = false;
                if self.class.is_empty() {
                    self.class = field(f, 1).to_string();
                }
                let level = number(f, 3) as u32;
                if self.level[0] == 0 {
                    self.level[0] = level;
                }
                self.level[1] = self.level[1].max(level);
            }
            "ES" => self.close_zone(r.ms),
            "TE" if self.full => self.teleport(r.fields),
            "Z" => self.enter_zone(r),
            "L" => {
                let level = number(f, 0) as u32;
                self.level[1] = self.level[1].max(level);
                self.levels.push(LevelAt {
                    level,
                    at: self.secs(r.ms),
                });
            }
            "X" => self.experience(r),
            "F" if field(f, 0) == "0" => self.combat.push((r.ms, number(f, 1))),
            "D" => self.death(r),
            "DR" => self.killing_blow(r),
            "K" => {
                let id = field(f, 0);
                if id != "p" && !id.is_empty() {
                    *self
                        .killed
                        .entry(self.name_of('n', id, "Creature"))
                        .or_default() += 1;
                }
            }
            "O" => {
                if let Ok(id) = field(f, 0).parse::<i64>() {
                    *self.looted.entry(id).or_default() += number(f, 1).max(1);
                }
            }
            "Q" if field(f, 0) == "2" => {
                let zone = self.zone.clone().unwrap_or_default();
                *self.quest_zones.entry(zone).or_default() += 1;
            }
            "LO" if field(f, 0) == "g" => self.gathers += 1,
            "T" if field(f, 0) == "1" => self.flights += 1,
            "AC" => self.achievements += 1,
            "C" => {
                if let Some(name) = self.f.names.get(&name_key('s', field(f, 0))) {
                    *self.casts_by_spell.entry(name.clone()).or_default() += 1;
                }
            }
            "TG" if self.spec.is_empty() => self.spec = self.spec_of(f),
            "EX" | "B" => {
                if r.code == "B" {
                    self.bosses += 1;
                }
                let ev = Ev {
                    cs: r.ms / 10,
                    code: r.code.to_string(),
                    fields: r.fields.to_vec(),
                };
                let (_, text) = event_parts(&ev, self.f.names);
                self.moments.push(Moment {
                    at: self.secs(r.ms),
                    kind: if r.code == "B" { "boss" } else { "discovery" },
                    text,
                    feat: None,
                    tier: None,
                });
            }
            _ => {}
        }
    }
}

fn merged(raw: Vec<Raw>) -> Vec<Raw> {
    let mut out: Vec<Raw> = Vec::new();
    for s in raw {
        match out.last_mut() {
            Some(l) if l.name == s.name || s.to - s.from < MIN_STRETCH_MS => l.to = s.to,
            _ => out.push(s),
        }
    }
    out
}

fn duration(secs: i64) -> String {
    let (h, m) = (secs / 3600, secs / 60 % 60);
    if h > 0 {
        format!("{h}h {m}m")
    } else if m > 0 {
        format!("{m}m")
    } else {
        format!("{secs}s")
    }
}

struct TitleInput<'a> {
    level: [u32; 2],
    zone: &'a str,
    dungeon: Option<(&'a str, i64)>,
    quests: i64,
    kills: i64,
    gathers: i64,
    miles: f64,
    zones: usize,
    played: i64,
}

fn title(t: &TitleInput) -> Title {
    let zone = if t.zone.is_empty() { "Azeroth" } else { t.zone };
    let (rule, text) = if t.level[1] > t.level[0] && t.level[0] > 0 {
        (
            "level",
            format!("Level {} to {} in {zone}", t.level[0], t.level[1]),
        )
    } else if let Some((name, runs)) = t.dungeon {
        (
            "dungeon",
            if runs <= 1 {
                format!("A run of {name}")
            } else {
                format!("{runs} runs of {name}")
            },
        )
    } else if t.quests >= QUEST_TITLE {
        ("quests", format!("{} quests in {zone}", t.quests))
    } else if t.kills >= KILL_TITLE {
        ("kills", format!("{} kills in {zone}", t.kills))
    } else if t.gathers >= GATHER_TITLE {
        ("gather", format!("{} gathers in {zone}", t.gathers))
    } else if t.miles >= TRAVEL_MILES && t.zones >= TRAVEL_ZONES {
        (
            "travel",
            format!("{:.1} miles across {} zones", t.miles, t.zones),
        )
    } else {
        ("time", format!("{} in {zone}", duration(t.played)))
    };
    Title { rule, text }
}

struct Scanned {
    zone: String,
    title: Title,
    stretches: Vec<Raw>,
    played: i64,
}

fn finish_scan(s: &mut Scan, spans: &[(i64, i64)]) -> Scanned {
    s.close_zone(s.last);
    let raw = std::mem::take(&mut s.stretches);
    let mut totals: Vec<(String, i64, bool)> = Vec::new();
    for r in &raw {
        match totals.iter_mut().find(|t| t.0 == r.name) {
            Some(t) => t.1 += r.to - r.from,
            None => totals.push((r.name.clone(), r.to - r.from, r.instance)),
        }
    }
    let top = totals
        .iter()
        .fold(None::<&(String, i64, bool)>, |best, t| match best {
            Some(b) if b.1 >= t.1 => Some(b),
            _ => Some(t),
        });
    let zone = top.map_or(String::new(), |t| t.0.clone());
    let dungeon = top
        .filter(|t| t.2)
        .map(|t| (t.0.as_str(), s.runs_by.get(&t.0).copied().unwrap_or(1)));
    let stretches = merged(raw);
    let played: i64 = spans.iter().map(|(a, b)| (b - a).max(0) / 1000).sum();
    let zones: HashSet<&str> = stretches.iter().map(|r| r.name.as_str()).collect();
    let input = TitleInput {
        level: s.level,
        zone: &zone,
        dungeon,
        quests: s.agg.completed as i64,
        kills: s.agg.kills as i64,
        gathers: s.gathers,
        miles: s.agg.distance() as f64 / MILE_YD,
        zones: zones.len(),
        played,
    };
    let title = title(&input);
    Scanned {
        zone,
        title,
        stretches,
        played,
    }
}

fn scan<'a>(f: &'a Facts<'a>, spans: &[(i64, i64)], full: bool) -> (Scan<'a>, Scanned) {
    let t0 = spans.iter().map(|s| s.0).min().unwrap_or(0);
    let mut s = Scan::new(f, full, t0);
    for r in spans_events(f, spans) {
        s.feed(&r);
    }
    let done = finish_scan(&mut s, spans);
    (s, done)
}

pub fn title_of(f: &Facts, span: (i64, i64)) -> Title {
    scan(f, &[span], false).1.title
}

fn bucket_layout(total_ms: i64) -> (i64, usize) {
    let mut secs = BUCKET_SECS;
    let count = |secs: i64| ((total_ms / 1000 + secs - 1) / secs).max(1);
    while count(secs) > MAX_BUCKETS {
        secs *= 2;
    }
    (secs, count(secs) as usize)
}

fn buckets(s: &Scan, end: i64) -> Buckets {
    let (secs, n) = bucket_layout(end - s.t0);
    let width = secs * 1000;
    let mut xp = vec![0i64; n];
    let mut combat = vec![0f64; n];
    for (ms, amount) in &s.xp_at {
        let i = (((ms - s.t0) / width) as usize).min(n - 1);
        xp[i] += amount;
    }
    for (ended, seconds) in &s.combat {
        let to = *ended - s.t0;
        let from = (to - seconds * 1000).max(0);
        let mut at = from;
        while at < to {
            let i = ((at / width) as usize).min(n - 1);
            let edge = ((i as i64 + 1) * width).min(to);
            combat[i] += (edge - at) as f64 / 1000.0;
            at = edge.max(at + 1);
        }
    }
    Buckets {
        secs,
        xp,
        combat: combat.iter().map(|c| c.round() as i64).collect(),
    }
}

fn ranked(map: HashMap<String, i64>) -> Vec<Counted> {
    let mut rows: Vec<Counted> = map
        .into_iter()
        .map(|(name, n)| Counted { name, n })
        .collect();
    rows.sort_by(|a, b| b.n.cmp(&a.n).then_with(|| a.name.cmp(&b.name)));
    rows.truncate(TOP);
    rows
}

fn top(s: &Scan) -> Top {
    let mut looted: Vec<Looted> = s
        .looted
        .iter()
        .map(|(id, n)| Looted {
            id: *id,
            name: s.name_of('i', &id.to_string(), "Item"),
            quality: 1,
            n: *n,
        })
        .collect();
    looted.sort_by(|a, b| b.n.cmp(&a.n).then_with(|| a.name.cmp(&b.name)));
    looted.truncate(TOP);
    let mut zones: Vec<ZoneCount> = s
        .quest_zones
        .iter()
        .map(|(zone, n)| ZoneCount {
            zone: zone.clone(),
            n: *n,
        })
        .collect();
    zones.sort_by(|a, b| b.n.cmp(&a.n).then_with(|| a.zone.cmp(&b.zone)));
    zones.truncate(TOP);
    Top {
        killed: ranked(s.killed.clone()),
        looted,
        killers: ranked(s.killers.clone()),
        quests_by_zone: zones,
    }
}

fn counters(s: &Scan) -> BTreeMap<&'static str, i64> {
    let a = &s.agg;
    let rows: [(&'static str, i64); 46] = [
        ("kills", a.kills as i64),
        ("casts", a.casts as i64),
        ("deaths", a.deaths as i64),
        ("foot", a.foot_yd),
        ("walk", a.walk_yd),
        ("mount", a.mount_yd),
        ("swim", a.swim_yd),
        ("fly", a.fly_yd),
        ("jumps", a.jumps as i64),
        ("xp", a.xp),
        ("earned", a.earned),
        ("spent", a.spent),
        ("loot", a.loot as i64),
        ("quests", a.completed as i64),
        ("dungeons", s.runs),
        ("bosses", s.bosses),
        ("combat", a.combat_s),
        ("flights", s.flights),
        ("levels", a.levels as i64),
        ("achievements", s.achievements),
        ("camps", a.camps as i64),
        ("camptime", a.camp_s),
        ("buffs", a.buffs as i64),
        ("footTime", a.foot_s),
        ("walkTime", a.walk_s),
        ("mountTime", a.mount_s),
        ("swimTime", a.swim_s),
        ("flyTime", a.fly_s),
        ("taxiTime", a.taxi_s),
        ("castTime", a.cast_s),
        ("afkTime", a.afk_s),
        ("restedTime", a.rested_s),
        ("groupTime", a.group_s),
        ("emotes", a.emotes as i64),
        ("saidLol", a.lols as i64),
        ("saidRip", a.rips as i64),
        ("damage", a.damage),
        ("healing", a.healing),
        ("damageTaken", a.taken),
        ("discoveries", a.discoveries as i64),
        ("repairs", a.repaired),
        ("npcTalks", a.npc_talks as i64),
        ("abandoned", a.abandoned as i64),
        ("crafted", a.crafted),
        ("honorKills", a.honor_kills),
        ("ccTaken", a.cc_s),
    ];
    rows.into_iter().filter(|(_, v)| *v != 0).collect()
}

fn thin(points: &[(f64, f64)], cap: usize) -> Vec<(f64, f64)> {
    if points.len() <= cap {
        return points.to_vec();
    }
    let stride = points.len().div_ceil(cap);
    let mut out: Vec<(f64, f64)> = points.iter().step_by(stride).copied().collect();
    if let Some(last) = points.last()
        && out.last() != Some(last)
    {
        out.push(*last);
    }
    out
}

fn route(legs_in: &[RawLeg]) -> Option<Route> {
    let lines: Vec<&RawLeg> = legs_in.iter().filter(|p| p.pts.len() >= 2).collect();
    let mut by_map: HashMap<u32, usize> = HashMap::new();
    for p in lines.iter().filter(|p| !p.dash) {
        *by_map.entry(p.map).or_default() += p.pts.len();
    }
    let map = by_map
        .iter()
        .max_by_key(|(m, n)| (**n, std::cmp::Reverse(**m)))
        .map(|(m, _)| *m)?;
    let legs: Vec<Leg> = lines
        .iter()
        .map(|p| {
            let pts = &p.pts;
            Leg {
                map: p.map,
                mode: if p.dash { "flight" } else { "ground" },
                points: thin(pts, LEG_POINTS)
                    .iter()
                    .flat_map(|(x, y)| [(x * 100.0).round() as i64, (y * 100.0).round() as i64])
                    .collect(),
            }
        })
        .collect();
    let on_map: Vec<&&RawLeg> = lines.iter().filter(|p| p.map == map).collect();
    let total: usize = on_map.iter().map(|p| p.pts.len()).sum();
    let stride = total.div_ceil(SHEET_POINTS).max(1);
    let mut kept: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut flights = Vec::new();
    for (i, p) in on_map.iter().enumerate() {
        let pts = &p.pts;
        let cap = (pts.len() / stride).max(2);
        kept.push(thin(pts, cap));
        if p.dash {
            flights.push(i);
        }
    }
    let all: Vec<(f64, f64)> = kept.iter().flatten().copied().collect();
    if all.len() < 2 {
        return None;
    }
    let range = |pick: fn(&(f64, f64)) -> f64| {
        let lo = all.iter().map(pick).fold(f64::MAX, f64::min);
        let hi = all.iter().map(pick).fold(f64::MIN, f64::max);
        (lo, (hi - lo).max(1.0))
    };
    let (x0, w) = range(|p| p.0);
    let (y0, h) = range(|p| p.1);
    let mut breaks = Vec::new();
    let mut points = Vec::new();
    let mut count = 0usize;
    for (i, leg) in kept.iter().enumerate() {
        if i > 0 {
            breaks.push(count);
        }
        for (x, y) in leg {
            points.push(((x - x0) / w * SHEET).round() as i64);
            points.push(((y - y0) / h * SHEET).round() as i64);
            count += 1;
        }
    }
    Some(Route {
        legs: Some(legs),
        sheet: Sheet {
            aspect: (w * MAP_ASPECT / h).clamp(0.4, 2.5),
            points,
            breaks,
            flights,
        },
    })
}

struct Totals {
    played: i64,
    xp: i64,
    kills: i64,
    quests: i64,
    earned: i64,
    distance: i64,
    deaths: i64,
    miles: f64,
}

impl Totals {
    fn per_hour(&self, n: i64) -> f64 {
        if self.played > 0 {
            n as f64 / (self.played as f64 / 3600.0)
        } else {
            0.0
        }
    }

    fn metrics(&self) -> [(&'static str, f64); 6] {
        [
            ("xp_h", self.per_hour(self.xp)),
            ("kills_h", self.per_hour(self.kills)),
            ("quests_h", self.per_hour(self.quests)),
            ("miles_h", self.miles / (self.played.max(1) as f64 / 3600.0)),
            ("deaths", self.deaths as f64),
            ("played", self.played as f64),
        ]
    }

    fn bests(&self) -> [(&'static str, f64); 6] {
        [
            ("xp", self.xp as f64),
            ("kills", self.kills as f64),
            ("quests", self.quests as f64),
            ("earned", self.earned as f64),
            ("distance", self.distance as f64),
            ("xp_h", self.per_hour(self.xp)),
        ]
    }
}

fn totals_of(a: &Agg, played: i64) -> Totals {
    Totals {
        played,
        xp: a.xp,
        kills: a.kills as i64,
        quests: a.completed as i64,
        earned: a.earned,
        distance: a.distance(),
        deaths: a.deaths as i64,
        miles: (a.foot_yd + a.walk_yd + a.mount_yd + a.swim_yd) as f64 / MILE_YD,
    }
}

fn totals(f: &Facts, from: i64, to: i64) -> Totals {
    let mut a = Agg::new();
    for r in spans_events(f, &[(from, to)]).filter(|r| r.code != "P") {
        a.apply(0, r.code, r.fields, r.ms);
    }
    totals_of(&a, (to - from).max(0) / 1000)
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let at = p * (sorted.len() - 1) as f64;
    let (lo, hi) = (at.floor() as usize, at.ceil() as usize);
    sorted[lo] + (sorted[hi] - sorted[lo]) * (at - lo as f64)
}

fn history(f: &Facts, start: i64, now: &Totals) -> (Vec<&'static str>, Option<Own>) {
    let mut before: Vec<(i64, i64)> = f
        .sessions
        .iter()
        .copied()
        .filter(|s| s.0 < start && s.1 - s.0 >= HISTORY_MIN_MS)
        .collect();
    before.sort();
    let before = &before[before.len().saturating_sub(HISTORY)..];
    if before.len() < HISTORY_NEEDED {
        return (Vec::new(), None);
    }
    let earlier: Vec<Totals> = before.iter().map(|s| totals(f, s.0, s.1)).collect();
    let bests = now
        .bests()
        .iter()
        .enumerate()
        .filter(|(i, (_, you))| {
            let top = earlier
                .iter()
                .map(|t| t.bests()[*i].1)
                .fold(f64::MIN, f64::max);
            *you > 0.0 && *you > top
        })
        .map(|(_, (key, _))| *key)
        .collect();
    let metrics = now
        .metrics()
        .iter()
        .enumerate()
        .map(|(i, (key, you))| {
            let mut values: Vec<f64> = earlier.iter().map(|t| t.metrics()[i].1).collect();
            values.sort_by(f64::total_cmp);
            (
                *key,
                Spread {
                    p10: percentile(&values, 0.10),
                    p25: percentile(&values, 0.25),
                    p50: percentile(&values, 0.50),
                    p75: percentile(&values, 0.75),
                    p90: percentile(&values, 0.90),
                    you: *you,
                },
            )
        })
        .collect();
    (
        bests,
        Some(Own {
            n: earlier.len(),
            metrics,
        }),
    )
}

/// The recap of one character over `spans` (ms, the sessions of one run), merged into one.
pub fn build(f: &Facts, spans: &[(i64, i64)]) -> Option<SessionRecap> {
    build_with(f, spans, true)
}

/// The same recap without its route, its comparison with earlier sessions and its top lists: what a card of the
/// sessions list draws.
pub fn lite(f: &Facts, span: (i64, i64)) -> Option<SessionRecap> {
    build_with(f, &[span], false)
}

fn build_with(f: &Facts, spans: &[(i64, i64)], full: bool) -> Option<SessionRecap> {
    let mut spans = spans.to_vec();
    spans.sort();
    let start = spans.first()?.0;
    let end = spans.iter().map(|s| s.1).max().unwrap_or(start);
    let (s, done) = scan(f, &spans, full);
    let (bests, own) = if full {
        history(f, start, &totals_of(&s.agg, done.played))
    } else {
        (Vec::new(), None)
    };
    let counters = counters(&s);
    let buckets = buckets(&s, end);
    let top = if full { top(&s) } else { Top::default() };
    let route = if full { route(&s.legs) } else { None };
    let mut moments = s.moments;
    moments.sort_by_key(|m| m.at);
    Some(SessionRecap {
        v: 1,
        character: f.character.to_string(),
        class: s.class,
        start: start / 1000,
        end: end / 1000,
        played: done.played,
        level: s.level,
        zone: done.zone,
        title: done.title,
        xp: s.xp,
        runs: s.runs,
        counters,
        buckets,
        zones: done
            .stretches
            .iter()
            .map(|r| Stretch {
                name: r.name.clone(),
                at: (r.from - start).max(0) / 1000,
                secs: (r.to - r.from) / 1000,
            })
            .collect(),
        levels: s.levels,
        moments,
        route,
        feats: Feats::default(),
        bests,
        own,
        top,
    })
}

const ADDON_SESSIONS: usize = 8;
const ADDON_TOP: usize = 5;
const ADDON_MIN_MS: i64 = 60_000;
const ADDON_MAX_BYTES: usize = 150_000;

/// The events of one character held in memory, in time order.
pub struct Log<'a> {
    recs: Vec<Rec<'a>>,
}

impl<'a> Log<'a> {
    pub fn new(recs: impl IntoIterator<Item = Rec<'a>>) -> Self {
        let mut recs: Vec<Rec<'a>> = recs.into_iter().collect();
        recs.sort_by_key(|r| r.ms);
        Log { recs }
    }

    /// The spans of the sessions, `S` to `ES`; one cut off without an `ES` ends at its last event.
    pub fn sessions(&self) -> Vec<(i64, i64)> {
        let mut out: Vec<(i64, i64)> = Vec::new();
        let mut open: Option<usize> = None;
        for r in &self.recs {
            match r.code {
                "S" => {
                    out.push((r.ms, r.ms));
                    open = Some(out.len() - 1);
                }
                "ES" => {
                    if let Some(i) = open.take() {
                        out[i].1 = r.ms;
                    }
                }
                _ => {
                    if let Some(i) = open {
                        out[i].1 = out[i].1.max(r.ms);
                    }
                }
            }
        }
        out
    }

    /// The names the events define: creatures, items and the rest of `name_kind`.
    pub fn names(&self) -> HashMap<String, String> {
        let mut names = HashMap::new();
        for r in &self.recs {
            if let Some(kind) = crate::events::name_kind(r.code)
                && let [id, name, ..] = r.fields
            {
                names.insert(name_key(kind, id), name.clone());
            }
        }
        names
    }
}

impl Events for Log<'_> {
    fn between(&self, from: i64, to: i64) -> Box<dyn Iterator<Item = Rec<'_>> + '_> {
        let start = self.recs.partition_point(|r| r.ms < from);
        Box::new(
            self.recs[start..]
                .iter()
                .take_while(move |r| r.ms <= to)
                .copied(),
        )
    }
}

fn for_addon(mut r: SessionRecap) -> SessionRecap {
    if let Some(route) = r.route.as_mut() {
        route.legs = None;
    }
    r.own = None;
    r.top.killed.truncate(ADDON_TOP);
    r.top.looted.truncate(ADDON_TOP);
    r.top.killers.truncate(ADDON_TOP);
    r.top.quests_by_zone.truncate(ADDON_TOP);
    r
}

/// The recaps the addon keeps for one character, newest first: its eight newest sessions of a minute or more, each
/// without `route.legs` and `own`, and with the lists of `top` cut to five.
pub fn for_character(character: &str, log: &Log) -> (Vec<SessionRecap>, Vec<Tally>) {
    let names = log.names();
    let sessions = log.sessions();
    let facts = Facts {
        character,
        events: log,
        names: &names,
        sessions: &sessions,
    };
    let all = tallies(&facts);
    let mut spans: Vec<(i64, i64)> = sessions
        .iter()
        .copied()
        .filter(|s| s.1 - s.0 >= ADDON_MIN_MS)
        .collect();
    spans.sort();
    let recaps = spans
        .iter()
        .rev()
        .take(ADDON_SESSIONS)
        .filter_map(|s| build(&facts, &[*s]))
        .map(|mut r| {
            crate::featscore::fill(&mut r, &all);
            for_addon(r)
        })
        .collect();
    (recaps, all)
}

fn render_recaps(chars: &[(String, Vec<serde_json::Value>)], made: i64) -> String {
    let mut root = serde_json::Map::new();
    for (name, recaps) in chars {
        root.insert(name.clone(), serde_json::Value::Array(recaps.clone()));
    }
    let mut out = format!("ScarletRecaps = {{\n  v = 1, made = {made},\n  chars = ");
    crate::overview::lua_write(&serde_json::Value::Object(root), 1, &mut out);
    out.push_str(",\n}\n");
    out
}

/// The text of Recaps.lua from each character's recaps (newest first). Under 150 KB: when it would not be, the oldest
/// recap of all goes first and every character keeps its newest.
pub fn recaps_lua(chars: &[(String, Vec<serde_json::Value>)], made: i64) -> String {
    let mut chars = chars.to_vec();
    let mut text = render_recaps(&chars, made);
    while text.len() > ADDON_MAX_BYTES {
        let start = |v: &serde_json::Value| v["start"].as_i64().unwrap_or(0);
        let Some(oldest) = chars
            .iter_mut()
            .filter(|(_, r)| r.len() > 1)
            .min_by_key(|(_, r)| r.last().map_or(0, start))
        else {
            break;
        };
        oldest.1.pop();
        text = render_recaps(&chars, made);
    }
    text
}

/// What one session adds up to, for counting feats: the session's counters, its casts by spell name, the talent tree
/// with the most points at login and the highest level.
#[derive(Clone)]
pub struct Tally {
    pub start: i64,
    pub played: i64,
    pub counters: BTreeMap<&'static str, i64>,
    pub casts: HashMap<String, i64>,
    pub spec: String,
    pub class: String,
    pub level: [u32; 2],
}

/// The tallies of every session of the character, oldest first.
pub fn tallies(f: &Facts) -> Vec<Tally> {
    let mut spans = f.sessions.to_vec();
    spans.sort();
    spans.into_iter().map(|s| tally(f, s)).collect()
}

pub fn tally(f: &Facts, span: (i64, i64)) -> Tally {
    let (s, done) = scan(f, &[span], false);
    Tally {
        start: span.0 / 1000,
        played: done.played,
        counters: counters(&s),
        casts: s.casts_by_spell,
        spec: s.spec,
        class: s.class,
        level: s.level,
    }
}

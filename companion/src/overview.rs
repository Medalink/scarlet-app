//! The character overview model (docs/OVERVIEW.md): one pass over a
//! character's events in time order gives the session rows, per-level and
//! per-zone splits, dungeon runs, groupmates, killers and the gold balance.
//! The desktop app shows the result; each export writes it to
//! `<char>/overview.json` and, for every character, to the addon's
//! `Digest.lua`.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};

/// One event: epoch milliseconds, code, fields. Both `archive::Event` and the
/// app's store rows lend themselves to this shape.
pub type Ev<'a> = (i64, &'a str, &'a [String]);

fn is_zero(n: &i64) -> bool {
    *n == 0
}

/// A session's (or a character's) share of one level.
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct LevelShare {
    #[serde(skip_serializing_if = "is_zero")]
    pub p: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub xk: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub xd: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub xq: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub xe: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub xo: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub k: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub dd: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub q: i64,
    /// Copper earned and spent, and dungeon and raid runs started, on this level.
    #[serde(skip_serializing_if = "is_zero")]
    pub g: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub s: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub r: i64,
}

impl LevelShare {
    fn add(&mut self, o: &LevelShare) {
        self.p += o.p;
        self.xk += o.xk;
        self.xd += o.xd;
        self.xq += o.xq;
        self.xe += o.xe;
        self.xo += o.xo;
        self.k += o.k;
        self.dd += o.dd;
        self.q += o.q;
        self.g += o.g;
        self.s += o.s;
        self.r += o.r;
    }

    fn xp_slot(&mut self, source: &str, dungeon: bool) -> &mut i64 {
        match source {
            "k" if dungeon => &mut self.xd,
            "k" => &mut self.xk,
            "q" => &mut self.xq,
            "e" => &mut self.xe,
            _ => &mut self.xo,
        }
    }
}

/// A session's (or a character's) share of one zone; `x` is all XP.
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct ZoneShare {
    #[serde(skip_serializing_if = "is_zero")]
    pub p: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub k: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub dd: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub x: i64,
}

/// One play session, keyed by `t` (the `S` line's time, epoch seconds).
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct SessionRow {
    pub t: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub p: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub l0: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub l1: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub xk: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub xd: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub xq: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub xe: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub xo: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub k: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub dd: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub q: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub r: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub g: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub s: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub m: Option<i64>,
    #[serde(skip_serializing_if = "is_zero")]
    pub z: i64,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub lv: BTreeMap<u32, LevelShare>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub zn: BTreeMap<String, ZoneShare>,
}

/// All sessions' share of one level, when it was reached and where most of
/// its time went.
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct LevelTotal {
    #[serde(flatten)]
    pub share: LevelShare,
    pub at: i64,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub zone: String,
    /// The ding into this level was seen and no unrecorded play has fallen inside it: its recorded
    /// time starts at the ding.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub whole: bool,
    /// `whole`, and the ding out of it was seen too. Only a complete level has a duration.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub complete: bool,
    /// The game's own played seconds at the ding into this level; 0 when no `/played` read followed it.
    #[serde(skip_serializing_if = "is_zero")]
    pub pd: i64,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct ZoneRow {
    pub name: String,
    #[serde(flatten)]
    pub share: ZoneShare,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct RunRow {
    pub inst: i64,
    pub name: String,
    pub n: i64,
    pub last: i64,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct MateRow {
    pub name: String,
    pub class: String,
    pub p: i64,
    pub n: i64,
    pub last: i64,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct NameCount {
    pub name: String,
    pub n: i64,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct GoldPoint {
    pub t: i64,
    pub m: i64,
}

/// The rested XP a character logged out with: the XP, what its level needs, whether it was
/// resting, and when (epoch seconds).
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct Rested {
    pub x: i64,
    pub m: i64,
    pub r: bool,
    pub t: i64,
}

/// One dungeon or raid at the level cap: runs started, time spent inside, bosses killed.
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct CapRun {
    pub name: String,
    pub n: i64,
    pub p: i64,
    pub b: i64,
}

/// What a character at the level cap has done there, from lines the per-level shares do not hold
/// (docs/OVERVIEW.md, "Max level"). Present once a session reported the cap (`S` field 14).
/// `level` is the cap; the rest is zero or empty until the character reaches it.
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct CapBlock {
    pub level: i64,
    /// When the character reached the cap (epoch seconds); 0 when before recording.
    pub reached: i64,
    /// Played seconds at the cap. With the ding into the cap seen (`reached`): the larger of the cap
    /// level's share and the client's last `/played` seconds on that level. Without it: the cap level's
    /// share alone, the time recorded since the character was first seen.
    pub p: i64,
    pub bosses: i64,
    pub loot: i64,
    pub honor: i64,
    pub hks: i64,
    pub skillups: i64,
    /// Legacy Challenges done and in all, and the Legacy points: the latest `AP` line.
    pub ldone: i64,
    pub ltotal: i64,
    pub lpoints: i64,
    /// The latest average item level.
    pub ilvl: i64,
    /// Reputation gained by faction, killers and death zones at the cap, top few of each.
    pub rep: Vec<NameCount>,
    pub runs: Vec<CapRun>,
    pub killers: Vec<NameCount>,
    pub dz: Vec<NameCount>,
}

/// Totals over the sessions the addon did not record itself (history brought in
/// from elsewhere): what its own counters, which start at install, lack. The keys
/// are the in-game Stats cards that history can fill.
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct Before {
    pub kills: i64,
    pub deaths: i64,
    pub dungeons: i64,
    pub bosses: i64,
    pub quests: i64,
    pub xp: i64,
    pub earned: i64,
    pub spent: i64,
    pub loot: i64,
    pub tracked: i64,
}

/// One character's overview: the Digest.lua per-character table.
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Overview {
    pub through: i64,
    pub class: String,
    pub race: String,
    pub faction: String,
    pub level: i64,
    pub xp: i64,
    pub xp_max: i64,
    pub zone: String,
    /// Lifetime: the game's played counter at the end of the last session; 0 when no `/played` read
    /// was recorded.
    pub played: i64,
    /// Observed: the seconds Scarlet recorded itself, summed over the sessions.
    pub recorded: i64,
    /// The level at the character's first recorded session; the levels below it were never seen.
    #[serde(skip_serializing_if = "is_zero")]
    pub first_level: i64,
    /// Stretches of play the sessions do not cover (another PC, the addon off, a lost
    /// SavedVariables), and the seconds the `/played` counter says they held.
    #[serde(skip_serializing_if = "is_zero")]
    pub gaps: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub gap_secs: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub money: Option<i64>,
    /// Newest first.
    pub sessions: Vec<SessionRow>,
    pub levels: BTreeMap<u32, LevelTotal>,
    pub zones: Vec<ZoneRow>,
    pub runs: Vec<RunRow>,
    pub runs_total: i64,
    pub mates: Vec<MateRow>,
    pub killers: Vec<NameCount>,
    pub death_zones: Vec<NameCount>,
    pub gold: Vec<GoldPoint>,
    /// Absent when every session was recorded by the addon.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<Before>,
    /// Absent until a session reported the level cap.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cap: Option<CapBlock>,
    /// The latest `RO` line; absent before one was recorded and at the level cap.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rested: Option<Rested>,
    /// History read from other addons (docs/FORMAT.md, "Imported history"). Never part of any
    /// number above.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imported: Option<crate::import::View>,
}

/// Played seconds at the level cap before the overview leads with max level data.
pub const CAP_FOCUS_SECONDS: i64 = 3600;

impl Overview {
    /// The cap level reported by the addon, when known.
    pub fn cap_level(&self) -> Option<i64> {
        self.cap.as_ref().map(|c| c.level).filter(|c| *c > 0)
    }

    /// At the cap, with an hour played there: the overview leads with max level data.
    pub fn max_focus(&self) -> bool {
        self.cap
            .as_ref()
            .is_some_and(|c| c.level > 0 && self.level >= c.level && c.p >= CAP_FOCUS_SECONDS)
    }
}

const MAX_ZONES: usize = 40;
const MAX_RUNS: usize = 10;
const MAX_MATES: usize = 20;
const MAX_NAMES: usize = 10;
const MAX_GOLD: usize = 30;
/// A dungeon left less than this long ago is the same run when re-entered.
const RUN_REJOIN_MS: i64 = 30 * 60 * 1000;
const GAP_TOLERANCE_MS: i64 = 120_000;
const XP_GAP_DIVISOR: i64 = 50;
const DING_READ_MS: i64 = 30 * 60 * 1000;

/// A whole number from a field; `?` and empty are none.
fn num(f: &[String], i: usize) -> Option<i64> {
    let s = f.get(i)?.trim();
    s.parse::<i64>()
        .ok()
        .or_else(|| s.parse::<f64>().ok().map(|x| x.round() as i64))
}

fn round_s(ms: i64) -> i64 {
    (ms + 500).div_euclid(1000)
}

/// The balance after each event, in copper. Anchors are the addon's own
/// balances (`S` field 13, `M` field 3) and the `ST` snapshot's copper; `M`
/// lines without a balance move the last known one, and lines before the first
/// anchor are walked back from it.
pub(crate) fn balances(evs: &[Ev]) -> Vec<Option<i64>> {
    let mut out = vec![None; evs.len()];
    let mut cur: Option<i64> = None;
    let mut first: Option<usize> = None;
    for (i, &(_, code, f)) in evs.iter().enumerate() {
        let anchor = match code {
            "S" => num(f, 12),
            "M" => num(f, 2),
            "ST" => num(f, 1),
            _ => None,
        };
        match anchor {
            Some(b) => {
                cur = Some(b);
                first.get_or_insert(i);
            }
            None if code == "M" => {
                if let (Some(c), Some(d)) = (cur, num(f, 0)) {
                    cur = Some(c + d);
                }
            }
            None => {}
        }
        out[i] = cur;
    }
    if let Some(i0) = first {
        let mut run = out[i0].unwrap_or(0);
        for j in (0..=i0).rev() {
            // A walk that goes below zero means money lines are missing.
            if run < 0 {
                break;
            }
            out[j] = Some(run);
            if evs[j].1 == "M" {
                run -= num(evs[j].2, 0).unwrap_or(0);
            }
        }
    }
    out
}

/// The session being read.
struct Open {
    row: SessionRow,
    start_ms: i64,
    /// Time up to which level and zone shares have been charged.
    charged_ms: i64,
    /// The time and index of the session's last line.
    end_ms: i64,
    end_idx: usize,
    level: u32,
    zone: String,
    dungeon: bool,
    lv: BTreeMap<u32, LevelShare>,
    zn: BTreeMap<String, ZoneShare>,
    mates_open: HashMap<String, i64>,
    /// The client's played counter (ms) at the session's start, from its first
    /// `/played` read: the read's total less the time since the session began.
    played_start_ms: Option<i64>,
    /// Its latest read: when (epoch ms) and the total (seconds).
    last_read: Option<(i64, i64)>,
    /// The addon recorded this session itself: its `S` line carries a version
    /// number. Brought-in history carries something else.
    native: bool,
    bosses: i64,
    loot: i64,
    gap_marked: bool,
}

/// What the session just closed leaves for the next one: where the client's
/// counter stood when it ended, and the level and zone its last seconds belong to.
struct Closed {
    played_end_ms: Option<i64>,
    end_ms: i64,
    level: u32,
    zone: String,
    native: bool,
}

impl Open {
    fn lv(&mut self) -> Option<&mut LevelShare> {
        (self.level > 0).then(|| self.lv.entry(self.level).or_default())
    }

    fn zn(&mut self) -> Option<&mut ZoneShare> {
        if self.zone.is_empty() {
            return None;
        }
        if !self.zn.contains_key(&self.zone) {
            self.zn.insert(self.zone.clone(), ZoneShare::default());
        }
        self.zn.get_mut(&self.zone)
    }
}

#[derive(Default)]
struct RunAcc {
    name: String,
    n: i64,
    last: i64,
}

#[derive(Default)]
struct MateAcc {
    p_ms: i64,
    n: i64,
    last: i64,
    /// 1-based number of the last session this mate was in.
    last_sess: usize,
}

/// Time, runs and bosses of one dungeon or raid at the cap.
#[derive(Default)]
struct CapRunAcc {
    n: i64,
    p_ms: i64,
    b: i64,
}

/// The lines at the level cap that the per-level shares do not hold.
#[derive(Default)]
struct CapAcc {
    bosses: i64,
    loot: i64,
    honor: i64,
    hks: i64,
    skillups: i64,
    ldone: i64,
    ltotal: i64,
    lpoints: i64,
    ilvl: i64,
    rep: HashMap<String, i64>,
    runs: HashMap<String, CapRunAcc>,
    killers: HashMap<String, i64>,
    dz: HashMap<String, i64>,
}

/// Names and how often, most first, at most `n`.
fn top_counts(m: HashMap<String, i64>, n: usize) -> Vec<NameCount> {
    let mut rows: Vec<NameCount> = m
        .into_iter()
        .filter(|(_, n)| *n > 0)
        .map(|(name, n)| NameCount { name, n })
        .collect();
    rows.sort_by(|a, b| b.n.cmp(&a.n).then(a.name.cmp(&b.name)));
    rows.truncate(n);
    rows
}

/// Each of the cap block's lists keeps this many rows.
const MAX_CAP_ROWS: usize = 5;

struct Builder {
    /// The level cap the addon last reported (`S` field 14).
    cap: Option<i64>,
    /// The client's seconds on the current level from its latest `/played` read at the cap.
    cap_tp: i64,
    acc: CapAcc,
    me: String,
    bal: Vec<Option<i64>>,
    sessions: Vec<SessionRow>,
    open: Option<Open>,
    closed: Option<Closed>,
    n_sess: usize,
    class: String,
    race: String,
    faction: String,
    level: u32,
    xp: i64,
    xp_max: i64,
    zone: String,
    tp: Option<i64>,
    before: Before,
    imported: bool,
    reached: BTreeMap<u32, i64>,
    level_zone_ms: HashMap<u32, HashMap<String, i64>>,
    runs: HashMap<i64, RunAcc>,
    runs_total: i64,
    run_inside: Option<i64>,
    run_last: Option<i64>,
    run_left: Option<i64>,
    mates: HashMap<String, MateAcc>,
    mate_class: HashMap<String, String>,
    killers: HashMap<String, i64>,
    death_zones: HashMap<String, i64>,
    deaths: crate::events::Deaths,
    moved: HashMap<usize, i64>,
    rested: Option<Rested>,
    gapped: std::collections::BTreeSet<u32>,
    gaps: i64,
    gap_ms: i64,
    dinged_at: BTreeMap<u32, i64>,
    ding_wait: Option<(u32, i64)>,
    first_level: i64,
}

impl Builder {
    fn new(me: &str, bal: Vec<Option<i64>>) -> Self {
        Self {
            cap: None,
            cap_tp: 0,
            acc: CapAcc::default(),
            me: me.to_string(),
            bal,
            sessions: Vec::new(),
            open: None,
            closed: None,
            n_sess: 0,
            class: String::new(),
            race: String::new(),
            faction: String::new(),
            level: 0,
            xp: 0,
            xp_max: 0,
            zone: String::new(),
            tp: None,
            before: Before::default(),
            imported: false,
            reached: BTreeMap::new(),
            level_zone_ms: HashMap::new(),
            runs: HashMap::new(),
            runs_total: 0,
            run_inside: None,
            run_last: None,
            run_left: None,
            mates: HashMap::new(),
            mate_class: HashMap::new(),
            killers: HashMap::new(),
            death_zones: HashMap::new(),
            deaths: Default::default(),
            moved: HashMap::new(),
            rested: None,
            gapped: std::collections::BTreeSet::new(),
            gaps: 0,
            gap_ms: 0,
            dinged_at: BTreeMap::new(),
            ding_wait: None,
            first_level: 0,
        }
    }

    fn mark_gap(&mut self, from: u32, new_level: u32, gap_ms: i64) {
        let (lo, hi) = (from.min(new_level), from.max(new_level));
        for l in lo..=hi.min(lo + 120) {
            if l > 0 {
                self.gapped.insert(l);
            }
        }
        self.gaps += 1;
        self.gap_ms += gap_ms.max(0);
    }

    fn starts_ahead(&self, level: u32, bar: Option<(i64, i64)>) -> bool {
        let Some(prev) = self.closed.as_ref() else {
            return false;
        };
        if prev.level == 0 || level == 0 {
            return false;
        }
        if level != prev.level {
            return level > prev.level;
        }
        let Some((x, m)) = bar else { return false };
        self.xp_max > 0 && m == self.xp_max && x > self.xp + (m / XP_GAP_DIVISOR).max(1)
    }

    fn start(&mut self, ms: i64, f: &[String]) {
        let field = |i: usize| f.get(i).cloned().unwrap_or_default();
        self.class = field(1);
        self.race = field(2);
        self.faction = field(4);
        let new_level = num(f, 3).filter(|l| *l > 0).map(|l| l as u32);
        let bar = num(f, 10).zip(num(f, 11));
        let ahead = new_level.is_some_and(|l| self.starts_ahead(l, bar));
        if let Some(l) = new_level {
            if ahead {
                let from = self.closed.as_ref().map_or(l, |c| c.level);
                self.mark_gap(from, l, 0);
            }
            if self.first_level == 0 {
                self.first_level = i64::from(l);
            }
            self.level = l;
        }
        self.ding_wait = None;
        if let Some((x, m)) = bar {
            self.xp = x;
            self.xp_max = m;
        }
        // Newer addons report the client's level cap in field 14; older lines have none.
        if let Some(c) = num(f, 13).filter(|c| *c > 0) {
            self.cap = Some(c);
        }
        self.n_sess += 1;
        let level = self.level;
        self.open = Some(Open {
            row: SessionRow {
                t: ms.div_euclid(1000),
                l0: i64::from(level),
                l1: i64::from(level),
                ..SessionRow::default()
            },
            start_ms: ms,
            charged_ms: ms,
            end_ms: ms,
            end_idx: 0,
            level,
            zone: String::new(),
            dungeon: false,
            lv: BTreeMap::new(),
            zn: BTreeMap::new(),
            mates_open: HashMap::new(),
            played_start_ms: None,
            last_read: None,
            native: !field(5).chars().next().is_some_and(|c| !c.is_ascii_digit()),
            bosses: 0,
            loot: 0,
            gap_marked: ahead,
        });
    }

    /// A `/played` read in the open session. The client keeps counting through
    /// a reload, which the session rows do not cover: the first read of a session
    /// gives the counter at its start, and what it stands above where the
    /// session before it ended is that gap. It is added to that session, to the
    /// level and the zone it ended in, up to the wall time between the two.
    fn played_read(&mut self, ms: i64, total_s: i64) {
        if let Some((level, ding_ms)) = self.ding_wait
            && level == self.level
            && (0..=DING_READ_MS).contains(&(ms - ding_ms))
        {
            self.dinged_at
                .entry(level)
                .or_insert_with(|| total_s - round_s(ms - ding_ms));
            self.ding_wait = None;
        }
        let Some(o) = self.open.as_mut() else { return };
        o.last_read = Some((ms, total_s));
        if o.played_start_ms.is_some() {
            return;
        }
        let start = total_s * 1000 - (ms - o.start_ms).max(0);
        o.played_start_ms = Some(start);
        let Some(prev) = self.closed.take() else {
            return;
        };
        let Some(end) = prev.played_end_ms else {
            return;
        };
        let wall = (o.start_ms - prev.end_ms).max(0);
        let gap = start - end;
        if gap > GAP_TOLERANCE_MS {
            let marked = std::mem::replace(&mut o.gap_marked, true);
            let level = o.row.l0 as u32;
            if marked {
                self.gap_ms += gap;
            } else {
                self.mark_gap(prev.level, level, gap);
            }
            return;
        }
        if gap < 1500 {
            return;
        }
        let credit = gap.min(wall + 1500);
        let secs = round_s(credit);
        let Some(row) = self.sessions.last_mut() else {
            return;
        };
        row.p += secs;
        if !prev.native {
            self.before.tracked += secs;
        }
        if prev.level > 0 {
            row.lv.entry(prev.level).or_default().p += secs;
        }
        if !prev.zone.is_empty()
            && let Some(z) = row.zn.get_mut(&prev.zone)
        {
            z.p += secs;
        }
        if prev.level > 0 && !prev.zone.is_empty() {
            *self
                .level_zone_ms
                .entry(prev.level)
                .or_default()
                .entry(prev.zone)
                .or_default() += credit;
        }
    }

    /// Charge the time since the last charge to the level and zone that were
    /// current during it.
    fn charge(&mut self, ms: i64) {
        let cap = self.cap;
        let Some(o) = self.open.as_mut() else { return };
        let dt = (ms - o.charged_ms).max(0);
        o.charged_ms = o.charged_ms.max(ms);
        if dt == 0 {
            return;
        }
        if o.dungeon && !o.zone.is_empty() && cap.is_some_and(|c| i64::from(o.level) >= c) {
            self.acc.runs.entry(o.zone.clone()).or_default().p_ms += dt;
        }
        if let Some(l) = o.lv() {
            l.p += dt;
        }
        if let Some(z) = o.zn() {
            z.p += dt;
        }
        if o.level > 0 && !o.zone.is_empty() {
            let m = self.level_zone_ms.entry(o.level).or_default();
            match m.get_mut(&o.zone) {
                Some(v) => *v += dt,
                None => {
                    m.insert(o.zone.clone(), dt);
                }
            }
        }
    }

    fn close(&mut self) {
        let Some(end) = self.open.as_ref().map(|o| o.end_ms) else {
            return;
        };
        self.charge(end);
        let Some(mut o) = self.open.take() else {
            return;
        };
        if self.run_inside.take().is_some() {
            self.run_left = Some(o.end_ms);
        }
        let sess = self.n_sess;
        for (name, join) in o.mates_open.drain() {
            let m = self.mates.entry(name).or_default();
            m.p_ms += (o.end_ms - join).max(0);
            m.last = m.last.max(o.end_ms.div_euclid(1000));
            if m.last_sess != sess {
                m.last_sess = sess;
                m.n += 1;
            }
        }
        self.closed = Some(Closed {
            played_end_ms: o
                .last_read
                .map(|(at, total)| total * 1000 + (o.end_ms - at).max(0)),
            end_ms: o.end_ms,
            level: o.level,
            zone: o.zone.clone(),
            native: o.native,
        });
        let mut row = o.row;
        row.p = round_s(o.end_ms - o.start_ms);
        if !o.native {
            let b = &mut self.before;
            b.kills += row.k;
            b.deaths += row.dd;
            b.dungeons += row.r;
            b.bosses += o.bosses;
            b.quests += row.q;
            b.xp += row.xk + row.xd + row.xq + row.xe + row.xo;
            b.earned += row.g;
            b.spent += row.s;
            b.loot += o.loot;
            b.tracked += row.p;
            self.imported = true;
        }
        row.m = self.bal.get(o.end_idx).copied().flatten();
        row.z = o.zn.len() as i64;
        for l in o.lv.values_mut() {
            l.p = round_s(l.p);
        }
        for z in o.zn.values_mut() {
            z.p = round_s(z.p);
        }
        row.lv = o.lv;
        row.zn = o.zn;
        self.sessions.push(row);
    }

    fn zone_changed(&mut self, ms: i64, f: &[String]) {
        let itype = f.get(2).map(String::as_str).unwrap_or("");
        let party = matches!(itype, "party" | "raid");
        let name = f.get(4).cloned().unwrap_or_default();
        if !name.is_empty() {
            self.zone = name.clone();
        }
        let cap = self.cap;
        let Some(o) = self.open.as_mut() else { return };
        o.dungeon = party;
        if !name.is_empty() {
            o.zone = name.clone();
            o.zn();
        }
        if !party {
            if self.run_inside.take().is_some() {
                self.run_left = Some(ms);
            }
            return;
        }
        let Some(id) = num(f, 1) else { return };
        if self.run_inside == Some(id) {
            return;
        }
        let same =
            self.run_last == Some(id) && self.run_left.is_some_and(|l| ms - l < RUN_REJOIN_MS);
        self.run_inside = Some(id);
        self.run_left = None;
        self.run_last = Some(id);
        if same {
            return;
        }
        o.row.r += 1;
        if let Some(l) = o.lv() {
            l.r += 1;
        }
        if !name.is_empty() && cap.is_some_and(|c| i64::from(o.level) >= c) {
            self.acc.runs.entry(name.clone()).or_default().n += 1;
        }
        self.runs_total += 1;
        let run = self.runs.entry(id).or_default();
        run.n += 1;
        run.last = ms.div_euclid(1000);
        if !name.is_empty() {
            run.name = name;
        }
    }

    fn event(&mut self, i: usize, ms: i64, code: &str, f: &[String]) {
        match code {
            "S" => {
                self.close();
                self.start(ms, f);
                if let Some(o) = self.open.as_mut() {
                    o.end_idx = i;
                }
                return;
            }
            "TP" => {
                if let Some(v) = num(f, 0) {
                    self.tp = Some(v);
                    self.played_read(ms, v);
                }
                // The client's seconds on the current level, read while at the cap, are the time there.
                if let (Some(secs), Some(c)) = (num(f, 1), self.cap)
                    && i64::from(self.level) >= c
                {
                    self.cap_tp = self.cap_tp.max(secs);
                }
                return;
            }
            "DP" => {
                if let (Some(name), Some(class)) = (f.get(1), f.get(2))
                    && !class.is_empty()
                    && class != "?"
                {
                    self.mate_class.insert(name.clone(), class.clone());
                }
                return;
            }
            _ => {}
        }
        let Some(o) = self.open.as_mut() else { return };
        o.end_ms = o.end_ms.max(ms);
        o.end_idx = i;
        let at_cap = self.cap.is_some_and(|c| i64::from(o.level) >= c);
        let acc = &mut self.acc;
        match code {
            "B" => {
                o.bosses += 1;
                if at_cap {
                    acc.bosses += 1;
                    if o.dungeon && !o.zone.is_empty() {
                        acc.runs.entry(o.zone.clone()).or_default().b += 1;
                    }
                }
            }
            "O" => {
                let n = num(f, 1).unwrap_or(1).max(1);
                o.loot += n;
                if at_cap {
                    acc.loot += n;
                }
            }
            "RP" if at_cap => {
                if let (Some(faction), Some(n)) = (f.first().filter(|s| !s.is_empty()), num(f, 1))
                    && n > 0
                {
                    *acc.rep.entry(faction.clone()).or_default() += n;
                }
            }
            "SK" if at_cap => acc.skillups += 1,
            "HN" if at_cap => acc.honor += num(f, 0).unwrap_or(0).max(0),
            "HK" if at_cap => acc.hks += num(f, 1).unwrap_or(0).max(0),
            // Latest values, at any level: Legacy points and challenges, and the item level.
            "RO" => {
                self.rested = match (num(f, 0), num(f, 1)) {
                    (Some(x), Some(m)) if m > 0 => Some(Rested {
                        x,
                        m,
                        r: num(f, 2) == Some(1),
                        t: ms.div_euclid(1000),
                    }),
                    _ => None,
                };
            }
            "AP" => {
                acc.lpoints = num(f, 0).unwrap_or(acc.lpoints);
                acc.ldone = num(f, 1).unwrap_or(acc.ldone);
                acc.ltotal = num(f, 2).unwrap_or(acc.ltotal);
            }
            "SX" if f.get(1).map(String::as_str) == Some("ilvlall") => {
                acc.ilvl = num(f, 2).unwrap_or(acc.ilvl);
            }
            "ST" => acc.ilvl = num(f, 10).filter(|v| *v > 0).unwrap_or(acc.ilvl),
            _ => {}
        }
        if !matches!(
            code,
            "ES" | "Z" | "L" | "X" | "K" | "D" | "Q" | "M" | "GM" | "DR"
        ) {
            return;
        }
        self.charge(ms);
        let cap = self.cap;
        let Some(o) = self.open.as_mut() else { return };
        let at_cap = cap.is_some_and(|c| i64::from(o.level) >= c);
        match code {
            "ES" => self.close(),
            "Z" => self.zone_changed(ms, f),
            "L" => {
                if let Some(l) = num(f, 0).filter(|l| *l > 0) {
                    if o.level != l as u32 {
                        self.ding_wait = Some((l as u32, ms));
                    }
                    o.level = l as u32;
                    o.row.l1 = l;
                    self.level = l as u32;
                    self.reached.entry(l as u32).or_insert(ms.div_euclid(1000));
                }
            }
            "X" => {
                let amount = num(f, 0).unwrap_or(0);
                let source = f.get(3).map(String::as_str).unwrap_or("");
                let dungeon = o.dungeon;
                let mut share = LevelShare::default();
                *share.xp_slot(source, dungeon) = amount;
                let r = &mut o.row;
                r.xk += share.xk;
                r.xd += share.xd;
                r.xq += share.xq;
                r.xe += share.xe;
                r.xo += share.xo;
                if let Some(l) = o.lv() {
                    l.xk += share.xk;
                    l.xd += share.xd;
                    l.xq += share.xq;
                    l.xe += share.xe;
                    l.xo += share.xo;
                }
                if let Some(z) = o.zn() {
                    z.x += amount;
                }
                if let (Some(x), Some(m)) = (num(f, 1), num(f, 2)) {
                    self.xp = x;
                    self.xp_max = m;
                }
            }
            "K" => {
                o.row.k += 1;
                if let Some(l) = o.lv() {
                    l.k += 1;
                }
                if let Some(z) = o.zn() {
                    z.k += 1;
                }
            }
            "D" => {
                if !self.deaths.died(ms) {
                    return;
                }
                o.row.dd += 1;
                if let Some(l) = o.lv() {
                    l.dd += 1;
                }
                if let Some(z) = o.zn() {
                    z.dd += 1;
                }
                if !o.zone.is_empty() {
                    *self.death_zones.entry(o.zone.clone()).or_default() += 1;
                    if at_cap {
                        *self.acc.dz.entry(o.zone.clone()).or_default() += 1;
                    }
                }
            }
            "Q" => {
                if f.first().map(String::as_str) == Some("2") {
                    o.row.q += 1;
                    if let Some(l) = o.lv() {
                        l.q += 1;
                    }
                }
            }
            "M" => {
                let moved = num(f, 1)
                    .filter(|t| *t != 0)
                    .or(self.moved.get(&i).copied());
                let net = num(f, 0).unwrap_or(0) - moved.unwrap_or(0);
                if net > 0 {
                    o.row.g += net;
                    if let Some(l) = o.lv() {
                        l.g += net;
                    }
                } else {
                    o.row.s += -net;
                    if let Some(l) = o.lv() {
                        l.s += -net;
                    }
                }
            }
            "GM" => self.mate(ms, f),
            "DR" if f.get(4).map(String::as_str) == Some("1") => {
                let killer = crate::events::environmental_killer(f)
                    .map(str::to_string)
                    .or_else(|| f.get(2).filter(|s| !s.is_empty() && *s != "?").cloned());
                if let Some(src) = killer.filter(|_| self.deaths.killing_blow(ms)) {
                    *self.killers.entry(src.clone()).or_default() += 1;
                    if at_cap {
                        *self.acc.killers.entry(src).or_default() += 1;
                    }
                }
            }
            _ => {}
        }
    }

    fn mate(&mut self, ms: i64, f: &[String]) {
        let Some(name) = f.get(1).filter(|n| !n.is_empty() && **n != self.me) else {
            return;
        };
        let Some(o) = self.open.as_mut() else { return };
        let sess = self.n_sess;
        let m = self.mates.entry(name.clone()).or_default();
        m.last = m.last.max(ms.div_euclid(1000));
        if m.last_sess != sess {
            m.last_sess = sess;
            m.n += 1;
        }
        if f.first().map(String::as_str) == Some("1") {
            o.mates_open.entry(name.clone()).or_insert(ms);
        } else {
            o.mates_open.remove(name);
            let secs = f.get(2).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
            m.p_ms += (secs.max(0.0) * 1000.0).round() as i64;
        }
    }

    fn finish(mut self) -> Overview {
        self.close();
        let money = self.bal.last().copied().flatten();
        let mut sessions = std::mem::take(&mut self.sessions);

        let mut levels: BTreeMap<u32, LevelTotal> = BTreeMap::new();
        let mut zones: HashMap<String, ZoneShare> = HashMap::new();
        for s in &sessions {
            for (l, share) in &s.lv {
                levels.entry(*l).or_default().share.add(share);
            }
            for (name, z) in &s.zn {
                let t = zones.entry(name.clone()).or_default();
                t.p += z.p;
                t.k += z.k;
                t.dd += z.dd;
                t.x += z.x;
            }
        }
        for (l, total) in levels.iter_mut() {
            total.at = self.reached.get(l).copied().unwrap_or(0);
            total.whole = self.reached.contains_key(l) && !self.gapped.contains(l);
            total.complete = total.whole && self.reached.contains_key(&(l + 1));
            total.pd = self
                .dinged_at
                .get(l)
                .copied()
                .filter(|p| *p > 0)
                .unwrap_or(0);
            total.zone = self
                .level_zone_ms
                .get(l)
                .and_then(|m| m.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))))
                .map(|(name, _)| name.clone())
                .unwrap_or_default();
        }

        // At the cap: what the shares do not hold, and the time there.
        let cap_block = self.cap.map(|c| {
            let at = u32::try_from(c).ok();
            let share_p = at.and_then(|k| levels.get(&k)).map_or(0, |t| t.share.p);
            let reached = at.and_then(|k| self.reached.get(&k)).copied().unwrap_or(0);
            let acc = std::mem::take(&mut self.acc);
            let mut runs: Vec<CapRun> = acc
                .runs
                .into_iter()
                .filter(|(_, r)| r.n > 0)
                .map(|(name, r)| CapRun {
                    name,
                    n: r.n,
                    p: round_s(r.p_ms),
                    b: r.b,
                })
                .collect();
            runs.sort_by(|a, b| b.n.cmp(&a.n).then(b.p.cmp(&a.p)).then(a.name.cmp(&b.name)));
            runs.truncate(MAX_CAP_ROWS);
            CapBlock {
                level: c,
                reached,
                p: if reached > 0 {
                    share_p.max(self.cap_tp)
                } else {
                    share_p
                },
                bosses: acc.bosses,
                loot: acc.loot,
                honor: acc.honor,
                hks: acc.hks,
                skillups: acc.skillups,
                ldone: acc.ldone,
                ltotal: acc.ltotal,
                lpoints: acc.lpoints,
                ilvl: acc.ilvl,
                rep: top_counts(acc.rep, MAX_CAP_ROWS),
                runs,
                killers: top_counts(acc.killers, MAX_CAP_ROWS),
                dz: top_counts(acc.dz, MAX_CAP_ROWS),
            }
        });

        let mut zone_rows: Vec<ZoneRow> = zones
            .into_iter()
            .map(|(name, share)| ZoneRow { name, share })
            .collect();
        zone_rows.sort_by(|a, b| b.share.p.cmp(&a.share.p).then(a.name.cmp(&b.name)));
        zone_rows.truncate(MAX_ZONES);

        let mut run_rows: Vec<RunRow> = self
            .runs
            .into_iter()
            .map(|(inst, r)| RunRow {
                inst,
                name: if r.name.is_empty() {
                    format!("Instance {inst}")
                } else {
                    r.name
                },
                n: r.n,
                last: r.last,
            })
            .collect();
        run_rows.sort_by(|a, b| {
            b.n.cmp(&a.n)
                .then(b.last.cmp(&a.last))
                .then(a.inst.cmp(&b.inst))
        });
        run_rows.truncate(MAX_RUNS);

        let mut mate_rows: Vec<MateRow> = self
            .mates
            .into_iter()
            .map(|(name, m)| MateRow {
                class: self.mate_class.get(&name).cloned().unwrap_or_default(),
                name,
                p: round_s(m.p_ms),
                n: m.n,
                last: m.last,
            })
            .collect();
        mate_rows.sort_by(|a, b| b.p.cmp(&a.p).then(b.n.cmp(&a.n)).then(a.name.cmp(&b.name)));
        mate_rows.truncate(MAX_MATES);

        let counted = |m: HashMap<String, i64>| {
            let mut rows: Vec<NameCount> = m
                .into_iter()
                .map(|(name, n)| NameCount { name, n })
                .collect();
            rows.sort_by(|a, b| b.n.cmp(&a.n).then(a.name.cmp(&b.name)));
            rows.truncate(MAX_NAMES);
            rows
        };

        // Sessions were read oldest first.
        let mut gold: Vec<GoldPoint> = sessions
            .iter()
            .filter_map(|s| s.m.map(|m| GoldPoint { t: s.t + s.p, m }))
            .collect();
        gold.drain(..gold.len().saturating_sub(MAX_GOLD));

        let sum_played: i64 = sessions.iter().map(|s| s.p).sum();
        // The client's counter when the last session ended, when it was read in it.
        let played_end = self.closed.as_ref().and_then(|c| c.played_end_ms);
        sessions.reverse();
        let through = sessions.first().map_or(0, |s| s.t);
        let level = sessions.first().map_or(0, |s| s.l1);
        // At the cap the client has no experience bar: whatever the bar last held is stale.
        let at_cap = self.cap.is_some_and(|c| level >= c);
        let (xp, xp_max) = if at_cap {
            (0, 0)
        } else {
            (self.xp, self.xp_max)
        };

        Overview {
            through,
            class: self.class,
            race: self.race,
            faction: self.faction,
            level,
            xp,
            xp_max,
            zone: self.zone,
            played: played_end.map_or_else(|| self.tp.unwrap_or(0), round_s),
            recorded: sum_played,
            first_level: self.first_level,
            gaps: self.gaps,
            gap_secs: round_s(self.gap_ms),
            money,
            sessions,
            levels,
            zones: zone_rows,
            runs: run_rows,
            runs_total: self.runs_total,
            mates: mate_rows,
            killers: counted(self.killers),
            death_zones: counted(self.death_zones),
            gold,
            before: self.imported.then_some(self.before),
            cap: cap_block,
            rested: if at_cap { None } else { self.rested },
            imported: None,
        }
    }
}

const TRANSFER_WINDOW_MS: i64 = 10_000;

fn name_key(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
        .to_lowercase()
}

fn is_own(who: &str, me: &str, own: &[String]) -> bool {
    let who = name_key(who);
    if who.is_empty() || who == name_key(me) {
        return false;
    }
    own.iter().any(|o| {
        let o = name_key(o);
        o == who || o.split('-').next() == Some(who.as_str())
    })
}

fn repair_transfers(evs: &[Ev], me: &str, own: &[String]) -> HashMap<usize, i64> {
    let mut moves: Vec<(i64, i64)> = Vec::new();
    for &(ms, code, f) in evs {
        let who = f.get(1).map(String::as_str).unwrap_or("");
        let copper = match (code, f.first().map(String::as_str)) {
            ("ML", Some("s")) => num(f, 2).map(|c| -c),
            ("ML", Some("m")) => num(f, 2),
            ("TR", Some("0")) if matches!(num(f, 2), Some(1 | 2)) => {
                Some(num(f, 4).unwrap_or(0) - num(f, 3).unwrap_or(0))
            }
            _ => None,
        };
        if let Some(c) = copper.filter(|c| *c != 0)
            && is_own(who, me, own)
        {
            moves.push((ms, c));
        }
    }
    let mut out = HashMap::new();
    if moves.is_empty() {
        return out;
    }
    let mut used = vec![false; moves.len()];
    for (i, &(ms, code, f)) in evs.iter().enumerate() {
        if code != "M" || num(f, 1).is_some_and(|t| t != 0) {
            continue;
        }
        let delta = num(f, 0).unwrap_or(0);
        let best = moves
            .iter()
            .enumerate()
            .filter(|(k, (t, c))| {
                !used[*k]
                    && (t - ms).abs() <= TRANSFER_WINDOW_MS
                    && ((*c < 0 && delta <= *c) || (*c > 0 && delta >= *c))
            })
            .min_by_key(|(_, (t, _))| (t - ms).abs());
        if let Some((k, &(_, c))) = best {
            used[k] = true;
            out.insert(i, c);
        }
    }
    out
}

/// Build one character's overview from their events, in time order. `me` is the character's own
/// Name-Surname; `own` holds the player's other characters: money mailed or traded between them
/// counts as a transfer, neither earned nor spent.
pub fn build_among<'a>(
    me: &str,
    events: impl IntoIterator<Item = Ev<'a>>,
    own: &[String],
) -> Overview {
    let evs: Vec<Ev> = events.into_iter().collect();
    let bal = balances(&evs);
    let mut b = Builder::new(me, bal);
    b.moved = repair_transfers(&evs, me, own);
    for (i, &(ms, code, f)) in evs.iter().enumerate() {
        b.event(i, ms, code, f);
    }
    b.finish()
}

// Level estimates ------------------------------------------------------------

const PACE_SECONDS: i64 = 36_000;
const MIN_FIT_LEVELS: usize = 5;
const MIN_LEVEL_PLAYED: i64 = 900;
const MAX_GROWTH: f64 = 1.05;
const DAY_WINDOW: i64 = 14;
const MAX_DAYS: f64 = 3650.0;
/// The cap used when no session reported one. `NEED` ends at level 60 whatever the cap.
const FALLBACK_CAP: i64 = 60;
/// `NEED[n - 1]` is the XP level `n` needs to reach level `n + 1`.
const NEED: [i64; 59] = [
    400, 900, 1400, 2100, 2800, 3600, 4500, 5400, 6500, 7600, 8800, 10100, 11400, 12900, 14400,
    16000, 17700, 19400, 21300, 23200, 25200, 27300, 29400, 31700, 34000, 36400, 38900, 41400,
    44300, 47400, 50800, 54500, 58600, 62800, 67100, 71600, 76100, 80800, 85700, 90700, 95800,
    101000, 106300, 111800, 117500, 123200, 129100, 135100, 141200, 147500, 153900, 160400, 167100,
    173900, 180800, 187900, 195000, 202300, 209800,
];

/// One session as the estimates read it: start, played seconds, all XP.
#[derive(Serialize, Clone, Copy, Debug)]
pub struct RecentRow {
    pub t: i64,
    pub p: i64,
    pub x: i64,
}

/// The rows the level estimates need, newest first: every session from the
/// last 14 days, and more until their played time adds up to 36,000 s.
/// `sessions` is newest first and complete.
pub fn recent_rows(sessions: &[SessionRow], now: i64) -> Vec<RecentRow> {
    let cutoff = now - DAY_WINDOW * 86_400;
    let mut out = Vec::new();
    let mut played = 0;
    for s in sessions {
        if s.t < cutoff && played >= PACE_SECONDS {
            break;
        }
        played += s.p;
        out.push(RecentRow {
            t: s.t,
            p: s.p,
            x: s.xk + s.xd + s.xq + s.xe + s.xo,
        });
    }
    out
}

/// One target level: the time to reach it and, when the pace allows, a date.
#[derive(Serialize, Clone, Debug)]
pub struct EtaTarget {
    pub level: i64,
    pub time: String,
    pub minutes: i64,
    pub date: String,
}

/// The level estimates of docs/OVERVIEW.md, ready to show.
#[derive(Serialize, Default, Clone, Debug)]
pub struct Eta {
    pub targets: Vec<EtaTarget>,
    /// The muted line under the targets; empty when there is nothing to say.
    pub basis: String,
}

/// "3d 4h", "1h 52m", "41m".
fn span(secs: f64) -> (String, i64) {
    let m = ((secs / 60.0 + 0.5).floor() as i64).max(1);
    let (d, h) = (m / 1440, m % 1440 / 60);
    let text = if d > 0 {
        format!("{d}d {h}h")
    } else if h > 0 {
        format!("{h}h {}m", m % 60)
    } else {
        format!("{m}m")
    };
    (text, m)
}

/// 12564 -> "12,564".
fn commas(n: i64) -> String {
    let digits = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// "Fri 28 Nov", with the year when it is not the year of `now`.
fn date_text(ts: i64, now: i64) -> String {
    use chrono::{Datelike, Local, TimeZone};
    let (Some(t), Some(n)) = (
        Local.timestamp_opt(ts, 0).single(),
        Local.timestamp_opt(now, 0).single(),
    ) else {
        return String::new();
    };
    let mut text = t.format("%a %-d %b").to_string();
    if t.year() != n.year() {
        text.push_str(&format!(" {}", t.year()));
    }
    text
}

/// Time to the next level, the next multiple of 10 and the level cap from the
/// level-weighted pace in `levels`, for a character at `level` with `xp` of
/// `xp_max` on the bar (`bar`). `rows` (newest first, the merged history) give the
/// days played; `first` is the start of the oldest recorded session. Same
/// rule as Overview.lua.
pub fn level_eta(
    levels: &BTreeMap<u32, LevelTotal>,
    rows: &[RecentRow],
    first: i64,
    level: i64,
    bar: (i64, i64),
    now: i64,
    cap: i64,
) -> Eta {
    let mut eta = Eta::default();
    // NEED ends at level 60: a longer climb is not estimated past it.
    let (xp, xp_max) = bar;
    let cap = cap.min(NEED.len() as i64 + 1);
    if !(1..cap).contains(&level) || xp_max <= 0 {
        return eta;
    }
    let day_start = now - DAY_WINDOW * 86_400;
    let day_played: i64 = rows.iter().filter(|r| r.t >= day_start).map(|r| r.p).sum();
    let Some(pace) = Pace::fit(levels, level) else {
        eta.basis =
            format!("Needs {MIN_FIT_LEVELS} complete levels of 15m or more with XP recorded");
        return eta;
    };
    let oldest = if first > 0 {
        first
    } else {
        rows.iter().map(|r| r.t).min().unwrap_or(now)
    };
    let covered = ((now - oldest) as f64 / 86_400.0).clamp(1.0, DAY_WINDOW as f64);
    let rate = day_played as f64 / covered;

    let mut targets = vec![level + 1];
    let major = (level / 10 + 1) * 10;
    if major != level + 1 && major < cap {
        targets.push(major);
    }
    if level + 1 != cap {
        targets.push(cap);
    }
    for target in targets {
        let mut secs = (xp_max - xp).max(0) as f64 / pace.rate(level);
        for n in level + 1..target {
            secs += NEED[(n - 1) as usize] as f64 / pace.rate(n);
        }
        let (time, minutes) = span(secs);
        let date = if rate > 0.0 && secs / rate <= MAX_DAYS {
            format!(
                "about {}",
                date_text((now as f64 + secs / rate * 86_400.0).floor() as i64, now)
            )
        } else {
            String::new()
        };
        eta.targets.push(EtaTarget {
            level: target,
            time,
            minutes,
            date,
        });
    }
    let hourly = commas((pace.rate(level) * 3600.0 + 0.5).floor() as i64);
    let basis = format!(
        "Based on {} played: {hourly} XP an hour",
        pace.levels_text()
    );
    eta.basis = if pace.flat() {
        basis
    } else {
        format!(
            "{basis} at level {level}, rising {:.1}% a level",
            (pace.slope.exp() - 1.0) * 100.0
        )
    };
    eta
}

/// XP per played second as a function of level: `exp(a + b * n)`.
struct Pace {
    a: f64,
    slope: f64,
    points: usize,
}

impl Pace {
    fn rate(&self, n: i64) -> f64 {
        (self.a + self.slope * n as f64).exp()
    }

    fn levels_text(&self) -> String {
        format!("{} complete levels", self.points)
    }

    fn flat(&self) -> bool {
        self.slope == 0.0
    }

    fn fit(levels: &BTreeMap<u32, LevelTotal>, level: i64) -> Option<Pace> {
        let mut pts: Vec<(f64, f64, f64)> = Vec::new();
        for (&n, t) in levels {
            if i64::from(n) > level || !t.complete {
                continue;
            }
            let s = &t.share;
            let x = s.xk + s.xd + s.xq + s.xe + s.xo;
            if s.p >= MIN_LEVEL_PLAYED && x > 0 {
                pts.push((f64::from(n), s.p as f64, (x as f64 / s.p as f64).ln()));
            }
        }
        if pts.len() < MIN_FIT_LEVELS {
            return None;
        }
        let w: f64 = pts.iter().map(|q| q.1).sum();
        let mean_n = pts.iter().map(|q| q.1 * q.0).sum::<f64>() / w;
        let mean_y = pts.iter().map(|q| q.1 * q.2).sum::<f64>() / w;
        let sxx: f64 = pts.iter().map(|q| q.1 * (q.0 - mean_n).powi(2)).sum();
        let sxy: f64 = pts
            .iter()
            .map(|q| q.1 * (q.0 - mean_n) * (q.2 - mean_y))
            .sum();
        let raw = sxy / sxx;
        let slope = raw.clamp(0.0, MAX_GROWTH.ln());
        let a = if slope == raw {
            mean_y - slope * mean_n
        } else {
            pts.iter().map(|q| q.1 * (q.2 - slope * q.0)).sum::<f64>() / w
        };
        Some(Pace {
            a,
            slope,
            points: pts.len(),
        })
    }
}

impl Overview {
    /// The start of the character's first recorded session; 0 when none.
    pub fn first_session(&self) -> i64 {
        self.sessions.last().map_or(0, |s| s.t)
    }

    /// The level estimates from this overview's sessions at `now`.
    pub fn eta(&self, now: i64) -> Eta {
        level_eta(
            &self.levels,
            &recent_rows(&self.sessions, now),
            self.first_session(),
            self.level,
            (self.xp, self.xp_max),
            now,
            self.cap_level().unwrap_or(FALLBACK_CAP),
        )
    }
}

// Digest.lua ---------------------------------------------------------------

const DIGEST_MAX_BYTES: usize = 200_000;
const DIGEST_SESSIONS: usize = 60;
const DIGEST_DAYS: i64 = 14;

fn lua_str(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\{:03}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn lua_key(k: &str, out: &mut String) {
    const KEYWORDS: [&str; 22] = [
        "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "if", "in",
        "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while", "goto",
    ];
    let ident = k
        .chars()
        .enumerate()
        .all(|(i, c)| c == '_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit()))
        && !k.is_empty()
        && !KEYWORDS.contains(&k);
    if ident {
        out.push_str(k);
    } else if !k.is_empty() && k.len() < 10 && k.bytes().all(|b| b.is_ascii_digit()) {
        out.push('[');
        out.push_str(k);
        out.push(']');
    } else {
        out.push('[');
        lua_str(k, out);
        out.push(']');
    }
}

/// Containers down to this depth get a line each, deeper ones stay inline.
const LUA_LINE_DEPTH: usize = 4;

pub(crate) fn lua_write(v: &Value, depth: usize, out: &mut String) {
    let nl = |out: &mut String, d: usize| {
        if depth < LUA_LINE_DEPTH {
            out.push('\n');
            for _ in 0..d {
                out.push_str("  ");
            }
        }
    };
    match v {
        Value::Object(m) => {
            out.push('{');
            for (k, v) in m.iter().filter(|(_, v)| !v.is_null()) {
                nl(out, depth + 1);
                lua_key(k, out);
                out.push('=');
                lua_write(v, depth + 1, out);
                out.push(',');
            }
            nl(out, depth);
            out.push('}');
        }
        Value::Array(a) => {
            out.push('{');
            for v in a {
                nl(out, depth + 1);
                lua_write(v, depth + 1, out);
                out.push(',');
            }
            nl(out, depth);
            out.push('}');
        }
        Value::String(s) => lua_str(s, out),
        Value::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => out.push_str(&i.to_string()),
            (None, Some(f)) if f.is_finite() => out.push_str(&f.to_string()),
            _ => out.push('0'),
        },
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Null => out.push_str("nil"),
    }
}

/// One of your other characters, from one character's side: seconds grouped, seconds online at
/// the same time, sessions grouped and the last time either happened.
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct AltRow {
    pub name: String,
    pub p: i64,
    pub o: i64,
    pub n: i64,
    pub last: i64,
}

/// One local day of a character's play: the day's midnight, played seconds, quests, copper
/// earned and spent, and dungeon and raid runs started.
#[derive(Serialize, Default, Clone, Debug)]
pub struct DayRow {
    pub d: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub p: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub q: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub g: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub s: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub r: i64,
}

const ALT_OVERLAP_MIN_SECS: i64 = 60;

fn local_midnight(t: i64) -> i64 {
    use chrono::{Local, TimeZone};
    Local
        .timestamp_opt(t, 0)
        .single()
        .and_then(|d| d.date_naive().and_hms_opt(0, 0, 0))
        .and_then(|n| Local.from_local_datetime(&n).earliest())
        .map_or(t - t.rem_euclid(86_400), |d| d.timestamp())
}

/// The last 14 local days of a character's sessions, newest first, each session on the day it
/// started. Days without play are left out.
pub fn recent_days(sessions: &[SessionRow], now: i64) -> Vec<DayRow> {
    let since = local_midnight(now - (DIGEST_DAYS - 1) * 86_400);
    let mut days: BTreeMap<i64, DayRow> = BTreeMap::new();
    for s in sessions
        .iter()
        .filter(|s| s.t >= since && s.t <= now + 86_400)
    {
        let d = local_midnight(s.t);
        let row = days.entry(d).or_insert_with(|| DayRow {
            d,
            ..DayRow::default()
        });
        row.p += s.p;
        row.q += s.q;
        row.g += s.g;
        row.s += s.s;
        row.r += s.r;
    }
    days.into_values().rev().collect()
}

fn online_together(a: &[SessionRow], b: &[SessionRow]) -> (i64, i64) {
    let spans = |rows: &[SessionRow]| {
        let mut v: Vec<(i64, i64)> = rows
            .iter()
            .filter(|s| s.p > 0)
            .map(|s| (s.t, s.t + s.p))
            .collect();
        v.sort_unstable();
        v
    };
    let (a, b) = (spans(a), spans(b));
    let (mut i, mut j, mut secs, mut last) = (0, 0, 0, 0);
    while i < a.len() && j < b.len() {
        let (lo, hi) = (a[i].0.max(b[j].0), a[i].1.min(b[j].1));
        if hi - lo >= ALT_OVERLAP_MIN_SECS {
            secs += hi - lo;
            last = last.max(hi);
        }
        if a[i].1 <= b[j].1 {
            i += 1;
        } else {
            j += 1;
        }
    }
    (secs, last)
}

/// A character's other characters of the same player: grouped time from its groupmate lines,
/// online-together time from the sessions, most together first.
pub fn alts_of(me: &str, ov: &Overview, all: &[(String, Overview)]) -> Vec<AltRow> {
    let mut rows: Vec<AltRow> = Vec::new();
    for (name, other) in all.iter().filter(|(n, _)| n != me) {
        let grouped = ov.mates.iter().find(|m| m.name == *name);
        let (o, ended) = online_together(&ov.sessions, &other.sessions);
        let row = AltRow {
            name: name.clone(),
            p: grouped.map_or(0, |m| m.p),
            o,
            n: grouped.map_or(0, |m| m.n),
            last: grouped.map_or(0, |m| m.last).max(ended),
        };
        if row.p > 0 || row.o > 0 || row.n > 0 {
            rows.push(row);
        }
    }
    rows.sort_by(|a, b| b.o.cmp(&a.o).then(b.p.cmp(&a.p)).then(a.name.cmp(&b.name)));
    rows
}

/// What Digest.lua says about the website: uploading, the link, the address and when each character last uploaded.
#[derive(Clone, Default)]
pub struct WebDigest {
    pub upload: bool,
    pub linked: bool,
    pub host: String,
    pub shared: BTreeMap<String, bool>,
    pub sync: i64,
    pub synced: BTreeMap<String, i64>,
}

static WEB: std::sync::Mutex<Option<WebDigest>> = std::sync::Mutex::new(None);

pub fn set_web(web: Option<WebDigest>) {
    *WEB.lock().unwrap_or_else(|e| e.into_inner()) = web;
}

pub fn web() -> Option<WebDigest> {
    WEB.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

static GROUP_BY_ACCOUNT: AtomicBool = AtomicBool::new(false);

/// Whether the digest asks the addon to list characters by game account. Off by default: the
/// field is written only when on.
pub fn set_group_by_account(on: bool) {
    GROUP_BY_ACCOUNT.store(on, Ordering::Relaxed);
}

fn render_digest(
    chars: &[(String, Overview)],
    made: i64,
    sessions: Option<usize>,
    accounts: &BTreeMap<String, Vec<String>>,
) -> String {
    let shown = web().filter(|w| w.upload);
    let mut root = serde_json::Map::new();
    for (name, ov) in chars {
        let mut view = ov.clone();
        let keep = match sessions {
            Some(n) => n,
            None => {
                let recent = view
                    .sessions
                    .iter()
                    .filter(|s| s.t >= made - DIGEST_DAYS * 86_400)
                    .count();
                recent.max(DIGEST_SESSIONS)
            }
        };
        // `recent` and `first` come from every session, before any trimming,
        // and are never trimmed.
        let recent = recent_rows(&ov.sessions, made);
        let first = ov.first_session();
        let alts = alts_of(name, ov, chars);
        let days = recent_days(&ov.sessions, made);
        view.mates
            .retain(|m| !chars.iter().any(|(n, _)| *n == m.name));
        view.sessions.truncate(keep);
        if let Ok(mut v) = serde_json::to_value(&view) {
            if let Some(m) = v.as_object_mut() {
                if let Some(w) = shown.as_ref().filter(|w| w.shared.get(name) == Some(&true)) {
                    m.insert(
                        "web".into(),
                        serde_json::json!({
                            "page": format!("{}/c/{name}", w.host),
                            "public": true,
                            "sync": w.synced.get(name).copied().unwrap_or(0),
                        }),
                    );
                }
                m.insert("first".into(), Value::from(first));
                if let Ok(r) = serde_json::to_value(&recent) {
                    m.insert("recent".into(), r);
                }
                if !alts.is_empty()
                    && let Ok(a) = serde_json::to_value(&alts)
                {
                    m.insert("alts".into(), a);
                }
                if !days.is_empty()
                    && let Ok(d) = serde_json::to_value(&days)
                {
                    m.insert("days".into(), d);
                }
            }
            root.insert(name.clone(), v);
        }
    }
    let mut out = format!("ScarletDigest = {{\n  v = 1, made = {made},\n");
    if let Some(w) = web() {
        let host = serde_json::to_value(&w.host).unwrap_or_default();
        out.push_str(&format!(
            "  web = {{ upload = {}, linked = {}, host = ",
            w.upload, w.linked
        ));
        lua_write(&host, 1, &mut out);
        out.push_str(&format!(", sync = {} }},\n", w.sync));
    }
    if GROUP_BY_ACCOUNT.load(Ordering::Relaxed) {
        out.push_str("  groupByAccount = true,\n");
    }
    if !accounts.is_empty()
        && let Ok(a) = serde_json::to_value(accounts)
    {
        out.push_str("  accounts = ");
        lua_write(&a, 1, &mut out);
        out.push_str(",\n");
    }
    out.push_str("  chars = ");
    lua_write(&Value::Object(root), 1, &mut out);
    out.push_str(",\n}\n");
    out
}

/// The text of Digest.lua for these characters (newest sessions first, the
/// last 60 or all from the last 14 days, whichever is more; plus `recent`
/// and `first` for the level estimates, which are never trimmed), each character's `alts` and
/// `days` and the top-level `accounts` (game account folder to its characters). Under 200 KB:
/// when it would not be, each character keeps fewer sessions. A character of yours is no
/// groupmate: it leaves `mates`.
pub fn digest_lua_with(
    chars: &[(String, Overview)],
    made: i64,
    accounts: &BTreeMap<String, Vec<String>>,
) -> String {
    let mut text = render_digest(chars, made, None, accounts);
    let mut keep = chars
        .iter()
        .map(|(_, o)| o.sessions.len())
        .max()
        .unwrap_or(0);
    let mut trimmed = false;
    while text.len() > DIGEST_MAX_BYTES && keep > 3 {
        trimmed = true;
        keep = if keep > DIGEST_SESSIONS {
            (keep / 2).max(DIGEST_SESSIONS)
        } else {
            keep * 2 / 3
        };
        text = render_digest(chars, made, Some(keep), accounts);
    }
    if trimmed && keep < DIGEST_SESSIONS {
        eprintln!(
            "scarlet: Digest.lua keeps only the newest {keep} sessions per character to stay under 200 KB; the in-game week can come up short"
        );
    }
    text
}

//! The character overview model (docs/OVERVIEW.md): one pass over a
//! character's events in time order gives the session rows, per-level and
//! per-zone splits, dungeon runs, groupmates, killers and the gold balance.
//! The desktop app shows the result; each export writes it to
//! `<char>/overview.json` and, for every character, to the addon's
//! `Digest.lua`.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

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
    pub played: i64,
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
}

const MAX_ZONES: usize = 40;
const MAX_RUNS: usize = 10;
const MAX_MATES: usize = 20;
const MAX_NAMES: usize = 10;
const MAX_GOLD: usize = 30;
/// A dungeon left less than this long ago is the same run when re-entered.
const RUN_REJOIN_MS: i64 = 30 * 60 * 1000;

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
fn balances(evs: &[Ev]) -> Vec<Option<i64>> {
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

struct Builder {
    me: String,
    bal: Vec<Option<i64>>,
    sessions: Vec<SessionRow>,
    open: Option<Open>,
    n_sess: usize,
    class: String,
    race: String,
    faction: String,
    level: u32,
    xp: i64,
    xp_max: i64,
    zone: String,
    tp: Option<i64>,
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
}

impl Builder {
    fn new(me: &str, bal: Vec<Option<i64>>) -> Self {
        Self {
            me: me.to_string(),
            bal,
            sessions: Vec::new(),
            open: None,
            n_sess: 0,
            class: String::new(),
            race: String::new(),
            faction: String::new(),
            level: 0,
            xp: 0,
            xp_max: 0,
            zone: String::new(),
            tp: None,
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
        }
    }

    fn start(&mut self, ms: i64, f: &[String]) {
        let field = |i: usize| f.get(i).cloned().unwrap_or_default();
        self.class = field(1);
        self.race = field(2);
        self.faction = field(4);
        if let Some(l) = num(f, 3).filter(|l| *l > 0) {
            self.level = l as u32;
        }
        if let (Some(x), Some(m)) = (num(f, 10), num(f, 11)) {
            self.xp = x;
            self.xp_max = m;
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
        });
    }

    /// Charge the time since the last charge to the level and zone that were
    /// current during it.
    fn charge(&mut self, ms: i64) {
        let Some(o) = self.open.as_mut() else { return };
        let dt = (ms - o.charged_ms).max(0);
        o.charged_ms = o.charged_ms.max(ms);
        if dt == 0 {
            return;
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
        let mut row = o.row;
        row.p = round_s(o.end_ms - o.start_ms);
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
        if !matches!(
            code,
            "ES" | "Z" | "L" | "X" | "K" | "D" | "Q" | "M" | "GM" | "DR"
        ) {
            return;
        }
        self.charge(ms);
        let Some(o) = self.open.as_mut() else { return };
        match code {
            "ES" => self.close(),
            "Z" => self.zone_changed(ms, f),
            "L" => {
                if let Some(l) = num(f, 0).filter(|l| *l > 0) {
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
                o.row.dd += 1;
                if let Some(l) = o.lv() {
                    l.dd += 1;
                }
                if let Some(z) = o.zn() {
                    z.dd += 1;
                }
                if !o.zone.is_empty() {
                    *self.death_zones.entry(o.zone.clone()).or_default() += 1;
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
                let net = num(f, 0).unwrap_or(0) - num(f, 1).unwrap_or(0);
                if net > 0 {
                    o.row.g += net;
                } else {
                    o.row.s += -net;
                }
            }
            "GM" => self.mate(ms, f),
            "DR" => {
                if f.get(4).map(String::as_str) == Some("1")
                    && let Some(src) = f.get(2).filter(|s| !s.is_empty() && *s != "?")
                {
                    *self.killers.entry(src.clone()).or_default() += 1;
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
            total.zone = self
                .level_zone_ms
                .get(l)
                .and_then(|m| m.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))))
                .map(|(name, _)| name.clone())
                .unwrap_or_default();
        }

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
        sessions.reverse();
        let through = sessions.first().map_or(0, |s| s.t);
        let level = sessions.first().map_or(0, |s| s.l1);

        Overview {
            through,
            class: self.class,
            race: self.race,
            faction: self.faction,
            level,
            xp: self.xp,
            xp_max: self.xp_max,
            zone: self.zone,
            played: self.tp.unwrap_or(sum_played),
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
        }
    }
}

/// Build one character's overview from their events, in time order. `me` is
/// the character's own Name-Surname.
pub fn build<'a>(me: &str, events: impl IntoIterator<Item = Ev<'a>>) -> Overview {
    let evs: Vec<Ev> = events.into_iter().collect();
    let bal = balances(&evs);
    let mut b = Builder::new(me, bal);
    for (i, &(ms, code, f)) in evs.iter().enumerate() {
        b.event(i, ms, code, f);
    }
    b.finish()
}

// Level estimates ------------------------------------------------------------

/// Played seconds `recent` covers, the days the calendar rate is measured
/// over, the played seconds the estimates need in all and on one level to
/// count it, and the largest rise a level.
const PACE_SECONDS: i64 = 36_000;
const MIN_PLAYED: i64 = 900;
const MIN_LEVEL_PLAYED: i64 = 900;
const MAX_GROWTH: f64 = 1.05;
const DAY_WINDOW: i64 = 14;
const MAX_DAYS: f64 = 3650.0;
const MAX_LEVEL: i64 = 60;
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

/// Time to the next level, the next multiple of 10 and level 60 from the
/// level-weighted pace in `levels`, for a character at `level` with `xp` of
/// `xp_max` on the bar. `rows` (newest first, the merged history) give the
/// days played; `first` is the start of the oldest recorded session. Same
/// rule as Overview.lua.
pub fn level_eta(
    levels: &BTreeMap<u32, LevelTotal>,
    rows: &[RecentRow],
    first: i64,
    level: i64,
    xp: i64,
    xp_max: i64,
    now: i64,
) -> Eta {
    let mut eta = Eta::default();
    if !(1..MAX_LEVEL).contains(&level) || xp_max <= 0 {
        return eta;
    }
    let day_start = now - DAY_WINDOW * 86_400;
    let day_played: i64 = rows.iter().filter(|r| r.t >= day_start).map(|r| r.p).sum();
    let Some(pace) = Pace::fit(levels, level) else {
        eta.basis = "No level times yet: they need 15m played with XP recorded".into();
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
    if major != level + 1 && major < MAX_LEVEL {
        targets.push(major);
    }
    if level + 1 != MAX_LEVEL {
        targets.push(MAX_LEVEL);
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
    /// Levels the fit used (all levels played when the pace is flat).
    points: usize,
}

impl Pace {
    fn rate(&self, n: i64) -> f64 {
        (self.a + self.slope * n as f64).exp()
    }

    /// "7 levels", or "1 level".
    fn levels_text(&self) -> String {
        match self.points {
            1 => "1 level".into(),
            n => format!("{n} levels"),
        }
    }

    fn flat(&self) -> bool {
        self.slope == 0.0
    }

    /// The level-weighted fit of docs/OVERVIEW.md; none without enough played
    /// time or XP. `levels` holds the per-level totals, `level` is the current one.
    fn fit(levels: &BTreeMap<u32, LevelTotal>, level: i64) -> Option<Pace> {
        let (mut played, mut gained) = (0i64, 0i64);
        let mut pts: Vec<(f64, f64, f64)> = Vec::new();
        for (&n, t) in levels {
            if i64::from(n) > level {
                continue;
            }
            let s = &t.share;
            let x = s.xk + s.xd + s.xq + s.xe + s.xo;
            played += s.p;
            gained += x;
            if s.p >= MIN_LEVEL_PLAYED && x > 0 {
                pts.push((f64::from(n), s.p as f64, (x as f64 / s.p as f64).ln()));
            }
        }
        if played < MIN_PLAYED || gained <= 0 {
            return None;
        }
        if pts.len() < 3 {
            return Some(Pace {
                a: (gained as f64 / played as f64).ln(),
                slope: 0.0,
                points: levels
                    .iter()
                    .filter(|(n, t)| i64::from(**n) <= level && t.share.p > 0)
                    .count(),
            });
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
            self.xp,
            self.xp_max,
            now,
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

fn lua_write(v: &Value, depth: usize, out: &mut String) {
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

fn render_digest(chars: &[(String, Overview)], made: i64, sessions: Option<usize>) -> String {
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
        view.sessions.truncate(keep);
        if let Ok(mut v) = serde_json::to_value(&view) {
            if let Some(m) = v.as_object_mut() {
                m.insert("first".into(), Value::from(first));
                if let Ok(r) = serde_json::to_value(&recent) {
                    m.insert("recent".into(), r);
                }
            }
            root.insert(name.clone(), v);
        }
    }
    let mut out = format!("ScarletDigest = {{\n  v = 1, made = {made},\n  chars = ");
    lua_write(&Value::Object(root), 1, &mut out);
    out.push_str(",\n}\n");
    out
}

/// The text of Digest.lua for these characters (newest sessions first, the
/// last 60 or all from the last 14 days, whichever is more; plus `recent`
/// and `first` for the level estimates, which are never trimmed). Under
/// 200 KB: when it would not be, each character keeps fewer sessions.
pub fn digest_lua(chars: &[(String, Overview)], made: i64) -> String {
    let mut text = render_digest(chars, made, None);
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
        text = render_digest(chars, made, Some(keep));
    }
    if trimmed && keep < DIGEST_SESSIONS {
        eprintln!(
            "scarlet: Digest.lua keeps only the newest {keep} sessions per character to stay under 200 KB; the in-game week can come up short"
        );
    }
    text
}

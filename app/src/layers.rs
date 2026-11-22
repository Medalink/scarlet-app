//! What the Map view's Survey look draws besides the walked lines: flights,
//! teleports, deaths, creatures met, gathering and fishing, NPC windows,
//! quests, discovered areas, camps and the subzones the player stood in.
//! Each character's lines run through one state machine in time order and
//! the results are kept per (character, map). Positions are the recorded
//! unit, map coordinates times 10000, as integers.

use serde::Serialize;
use std::collections::{HashMap, HashSet};

type Pt = [i32; 2];

/// A fishing channel ending this soon before a loot window makes it a catch.
const CATCH_MS: i64 = 6000;

#[derive(Default)]
pub struct MapLayers {
    /// The zone's own name, so a subzone named like it can be dropped.
    zone: String,
    flights: Vec<[Pt; 2]>,
    teleports: Vec<[Pt; 2]>,
    deaths: Vec<Pt>,
    met: HashMap<Pt, u32>,
    gather: Vec<Pt>,
    fishing: Vec<Pt>,
    npcs: HashMap<String, Npc>,
    quests: Vec<(i32, i32, u8)>,
    found: Vec<(String, Pt)>,
    camps: Vec<Pt>,
    places: HashMap<String, Place>,
}

struct Npc {
    kind: String,
    x: i64,
    y: i64,
    n: i64,
}

#[derive(Default)]
struct Place {
    n: u32,
    x: i64,
    y: i64,
    pts: Vec<Pt>,
}

pub type Layers = HashMap<(u32, u32), MapLayers>;

/// One character's state while its lines are read in time order. A session
/// start forgets where the character was and whether it was flying.
pub struct Feed {
    ch: u32,
    fish: HashSet<String>,
    flying: Option<(Option<u32>, Pt)>,
    here: Option<(u32, Pt)>,
    sub: Option<(u32, String)>,
    fished_at: i64,
    deaths: scarlet::events::Deaths,
}

fn int(f: &[String], i: usize) -> Option<i32> {
    f.get(i)?.parse::<f64>().ok().map(|v| v.round() as i32)
}

fn map_id(f: &[String], i: usize) -> Option<u32> {
    f.get(i)?.parse().ok()
}

impl Feed {
    /// `fish` is the ids of the character's spells named Fishing.
    pub fn new(ch: u32, fish: HashSet<String>) -> Feed {
        Feed {
            ch,
            fish,
            flying: None,
            here: None,
            sub: None,
            fished_at: i64::MIN / 2,
            deaths: Default::default(),
        }
    }

    fn at<'a>(&self, out: &'a mut Layers, map: u32) -> &'a mut MapLayers {
        out.entry((self.ch, map)).or_default()
    }

    pub fn line(&mut self, out: &mut Layers, ms: i64, code: &str, f: &[String]) {
        match code {
            "S" => {
                self.flying = None;
                self.here = None;
            }
            "Z" => {
                let map = map_id(f, 0);
                self.sub = map.map(|m| {
                    (
                        m,
                        f.get(5).map(|s| s.trim().to_string()).unwrap_or_default(),
                    )
                });
                if let (Some(m), Some(zone)) =
                    (map, f.get(4).map(|z| z.trim()).filter(|z| !z.is_empty()))
                {
                    self.at(out, m).zone = zone.to_string();
                }
                if self.here.is_some_and(|(h, _)| Some(h) != map) {
                    self.here = None;
                }
            }
            "P" => {
                let (Some(m), Some(x), Some(y)) = (map_id(f, 0), int(f, 1), int(f, 2)) else {
                    return;
                };
                self.here = Some((m, [x, y]));
                if self.flying.is_some() {
                    return;
                }
                if let Some((sm, name)) = &self.sub
                    && *sm == m
                    && !name.is_empty()
                {
                    let p = self.at(out, m).places.entry(name.clone()).or_default();
                    p.n += 1;
                    p.x += x as i64;
                    p.y += y as i64;
                    p.pts.push([x, y]);
                }
            }
            "T" => {
                let (m, x, y) = (map_id(f, 1), int(f, 2), int(f, 3));
                if f.first().map(String::as_str) == Some("1") {
                    self.flying = Some((m, [x.unwrap_or(0), y.unwrap_or(0)]));
                } else if let (Some((fm, from)), Some(m), Some(x), Some(y)) =
                    (self.flying.take(), m, x, y)
                    && fm == Some(m)
                {
                    self.at(out, m).flights.push([from, [x, y]]);
                }
            }
            "TE" => {
                if let (Some(a), Some(b)) = (map_id(f, 3), map_id(f, 6))
                    && a == b
                    && let (Some(fx), Some(fy), Some(tx), Some(ty)) =
                        (int(f, 4), int(f, 5), int(f, 7), int(f, 8))
                {
                    self.at(out, a).teleports.push([[fx, fy], [tx, ty]]);
                }
            }
            "D" => {
                self.deaths.died(ms);
            }
            "DL" => {
                if self.deaths.repeated() {
                    return;
                }
                if let (Some(m), Some(x), Some(y)) = (map_id(f, 0), int(f, 1), int(f, 2)) {
                    self.at(out, m).deaths.push([x, y]);
                }
            }
            // NS carries its cell after nine fields, NP after three.
            "NS" | "NP" => {
                let o = if code == "NS" { 8 } else { 2 };
                if let (Some(m), Some(cx), Some(cy)) = (map_id(f, o), int(f, o + 1), int(f, o + 2))
                {
                    *self.at(out, m).met.entry([cx, cy]).or_default() += 1;
                }
            }
            "CH" => {
                if f.first().is_some_and(|id| self.fish.contains(id)) {
                    self.fished_at = ms;
                }
            }
            _ => self.here_line(out, ms, code, f),
        }
    }

    /// Lines that happen where the character last stood.
    fn here_line(&mut self, out: &mut Layers, ms: i64, code: &str, f: &[String]) {
        let Some((m, p)) = self.here else { return };
        let field = |i: usize| f.get(i).map(String::as_str).unwrap_or("");
        match code {
            "LO" if field(0) == "g" && field(4) != "1" => {
                let l = self.at(out, m);
                if ms - self.fished_at < CATCH_MS {
                    l.fishing.push(p);
                } else {
                    l.gather.push(p);
                }
            }
            "NI" => {
                let e = self
                    .at(out, m)
                    .npcs
                    .entry(field(1).to_string())
                    .or_insert_with(|| Npc {
                        kind: field(0).to_string(),
                        x: 0,
                        y: 0,
                        n: 0,
                    });
                e.x += p[0] as i64;
                e.y += p[1] as i64;
                e.n += 1;
            }
            "Q" if matches!(field(0), "1" | "2") => {
                let kind = if field(0) == "1" { 1 } else { 2 };
                self.at(out, m).quests.push((p[0], p[1], kind));
            }
            "EX" => self.at(out, m).found.push((field(0).to_string(), p)),
            "CP" if field(0) == "1" && field(2) != "1" => self.at(out, m).camps.push(p),
            _ => {}
        }
    }

    /// A line another addon recorded: gathering nodes and catches.
    pub fn addon(&self, out: &mut Layers, key: &str, code: &str, f: &[String]) {
        match (key, code) {
            ("gm", "N") => {
                if let (Some(m), Some(x), Some(y)) = (map_id(f, 0), int(f, 1), int(f, 2)) {
                    let l = self.at(out, m);
                    if f.get(3).map(String::as_str) == Some("Fishing") {
                        l.fishing.push([x, y]);
                    } else {
                        l.gather.push([x, y]);
                    }
                }
            }
            ("fl", "C") => {
                if let (Some(m), Some(x), Some(y)) = (map_id(f, 5), int(f, 6), int(f, 7)) {
                    self.at(out, m).fishing.push([x, y]);
                }
            }
            _ => {}
        }
    }
}

#[derive(Serialize)]
pub struct Found {
    name: String,
    x: i32,
    y: i32,
}

#[derive(Serialize)]
pub struct PlaceOut {
    name: String,
    n: u32,
    x: i32,
    y: i32,
}

#[derive(Serialize, Default)]
pub struct LayersOut {
    flights: Vec<[Pt; 2]>,
    teleports: Vec<[Pt; 2]>,
    deaths: Vec<Pt>,
    met: Vec<(i32, i32, u32)>,
    gather: Vec<Pt>,
    fishing: Vec<Pt>,
    npcs: Vec<(i32, i32, String)>,
    quests: Vec<(i32, i32, u8)>,
    found: Vec<Found>,
    camps: Vec<Pt>,
    places: Vec<PlaceOut>,
}

/// Marks that fall on the same spot (within `grid`, of the same kind) are kept once.
fn once<T>(list: Vec<T>, grid: i32, at: impl Fn(&T) -> (i32, i32, i32)) -> Vec<T> {
    let mut seen = HashSet::new();
    list.into_iter()
        .filter(|t| {
            let (x, y, k) = at(t);
            let r = |v: i32| (v as f64 / grid as f64).round() as i32;
            seen.insert((r(x), r(y), k))
        })
        .collect()
}

/// One map's layers for one character, or for all of them when `ch` is None.
pub fn collect(layers: &Layers, map: u32, ch: Option<u32>) -> LayersOut {
    let mut keys: Vec<&(u32, u32)> = layers
        .keys()
        .filter(|(c, m)| *m == map && ch.is_none_or(|cc| cc == *c))
        .collect();
    keys.sort();
    let mut o = LayersOut::default();
    let mut met: HashMap<Pt, u32> = HashMap::new();
    let mut npcs: HashMap<&str, (&str, i64, i64, i64)> = HashMap::new();
    let mut places: HashMap<&str, Place> = HashMap::new();
    let mut found: Vec<(String, Pt)> = Vec::new();
    let mut zones: HashSet<&str> = HashSet::new();
    let (mut gather, mut fishing, mut quests, mut camps) = (vec![], vec![], vec![], vec![]);
    for k in keys {
        let l = &layers[k];
        zones.insert(&l.zone);
        o.flights.extend(&l.flights);
        o.teleports.extend(&l.teleports);
        o.deaths.extend(&l.deaths);
        gather.extend(&l.gather);
        fishing.extend(&l.fishing);
        quests.extend(&l.quests);
        camps.extend(&l.camps);
        for (c, n) in &l.met {
            *met.entry(*c).or_default() += n;
        }
        for (id, n) in &l.npcs {
            let e = npcs.entry(id).or_insert((&n.kind, 0, 0, 0));
            e.1 += n.x;
            e.2 += n.y;
            e.3 += n.n;
        }
        for (name, p) in &l.places {
            let e = places.entry(name).or_default();
            e.n += p.n;
            e.x += p.x;
            e.y += p.y;
            e.pts.extend(&p.pts);
        }
        for (name, p) in &l.found {
            match found.iter_mut().find(|(n, _)| n == name) {
                Some(f) => f.1 = *p,
                None => found.push((name.clone(), *p)),
            }
        }
    }
    o.gather = once(gather, 40, |p| (p[0], p[1], 0));
    o.fishing = once(fishing, 40, |p| (p[0], p[1], 0));
    o.quests = once(quests, 40, |q| (q.0, q.1, q.2 as i32));
    o.camps = once(camps, 80, |p| (p[0], p[1], 0));
    o.met = met.into_iter().map(|([x, y], n)| (x, y, n)).collect();
    o.met.sort();
    let mut npcs: Vec<_> = npcs
        .into_values()
        .map(|(kind, x, y, n)| ((x / n) as i32, (y / n) as i32, kind.to_string()))
        .collect();
    npcs.sort();
    o.npcs = npcs;
    o.found = found
        .into_iter()
        .map(|(name, p)| Found {
            name,
            x: p[0],
            y: p[1],
        })
        .collect();
    // A place is lettered at the recorded point nearest the middle of its points, so the dot
    // sits where the player stood. A subzone named like its zone says nothing and is dropped.
    o.places = places
        .into_iter()
        .filter(|(name, _)| !zones.contains(name))
        .map(|(name, p)| {
            let (cx, cy) = (p.x as f64 / p.n as f64, p.y as f64 / p.n as f64);
            let best = p
                .pts
                .iter()
                .min_by(|a, b| {
                    let d = |q: &&Pt| (q[0] as f64 - cx).powi(2) + (q[1] as f64 - cy).powi(2);
                    d(a).total_cmp(&d(b))
                })
                .copied()
                .unwrap_or([0, 0]);
            PlaceOut {
                name: name.to_string(),
                n: p.n,
                x: best[0],
                y: best[1],
            }
        })
        .collect();
    o.places
        .sort_by(|a, b| b.n.cmp(&a.n).then_with(|| a.name.cmp(&b.name)));
    o
}

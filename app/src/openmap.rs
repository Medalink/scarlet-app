//! Map art read from the OpenMap addon the player has installed (https://github.com/openmap-world/openmap).
//!
//! OpenMap bakes the art of every zone into 512 px world tiles (`Data\Forever\ZoneArt.lua` lists them,
//! `Media\Textures\Forever\MapZones\<i>_<j>.blp` holds them). This reads those files in place, never
//! writes to the addon's folder, and cuts one picture per map out of them. None of the addon's art or
//! data is copied into this repository: the only table kept here is the game's own map rectangles
//! (`openmap_maps.json`, made by `tools/openmap-maps.mjs`). The art is Blizzard's.
//!
//! This module knows nothing of Tauri, so a second entry point can cut maps on its own.

use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Bumped when the cutting changes, so caches made by an older one are not reused.
pub const CUT: u32 = 1;
/// Where the pictures go; the OpenMap TOC version and `CUT` name the folder below it.
const CACHE: &str = "openmap";
/// What shows where no tile has art (open sea beyond the baked zones): the sea the tiles fade into.
const SEA: [f32; 3] = [124.0, 136.0, 132.0];
/// Sharpest mip used; a world-sized picture is cut from tiles this far down.
const MAX_MIP: u32 = 6;
/// Decoded tiles kept between maps; one decoded 512 px tile is 1 MiB.
const KEEP: usize = 96;
const WEBP_QUALITY: f32 = 90.0;

#[derive(Deserialize)]
struct Table {
    world: World,
    maps: Vec<Entry>,
}

#[derive(Deserialize)]
struct World {
    w: f64,
    h: f64,
    rows: Vec<Rect>,
}

/// One row of the client's UiMapAssignment: the part of a map's sheet (`min`..`max`) that lies
/// on world continent `c` between `r` = [x0, y0, x1, y1] in yards.
#[derive(Deserialize, Clone)]
struct Rect {
    c: u32,
    min: [f64; 2],
    max: [f64; 2],
    r: [f64; 4],
}

#[derive(Deserialize)]
pub struct Entry {
    pub id: u32,
    pub name: String,
    pub w: u32,
    pub h: u32,
    /// A city: OpenMap's tiles leave its art blank, so there is nothing to cut.
    #[serde(default)]
    pub city: bool,
    #[serde(flatten)]
    rect: Option<Rect>,
}

fn table() -> &'static Table {
    static T: std::sync::OnceLock<Table> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(include_str!("openmap_maps.json")).expect("openmap_maps.json is valid")
    })
}

/// The maps that can be cut out of OpenMap's tiles.
pub fn maps() -> &'static [Entry] {
    &table().maps
}

/// An OpenMap install: its folder and the version its TOC names.
#[derive(Clone)]
pub struct Install {
    dir: PathBuf,
    pub version: String,
}

impl Install {
    /// OpenMap inside a WoW Forever install, when its files are all there.
    pub fn find(wow: &Path) -> Option<Install> {
        let dir = wow
            .join("_forever_")
            .join("Interface")
            .join("AddOns")
            .join("OpenMap");
        let toc = fs::read_to_string(dir.join("OpenMap.toc")).ok()?;
        if !dir.join("Data/Forever/ZoneArt.lua").is_file()
            || !dir.join("Media/Textures/Forever/MapZones").is_dir()
        {
            return None;
        }
        let version = toc
            .lines()
            .find_map(|l| l.strip_prefix("## Version:"))
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "unknown".into());
        Some(Install { dir, version })
    }

    fn zone_art(&self) -> Result<ZoneArt, String> {
        let text = fs::read_to_string(self.dir.join("Data/Forever/ZoneArt.lua"))
            .map_err(|e| format!("ZoneArt.lua: {e}"))?;
        ZoneArt::parse(&text)
    }

    fn tile_path(&self, i: u32, j: u32) -> PathBuf {
        self.dir
            .join("Media/Textures/Forever/MapZones")
            .join(format!("{i}_{j}.blp"))
    }

    /// Where this install's pictures are cached under the app's data folder.
    pub fn cache_dir(&self, data: &Path) -> PathBuf {
        let version: String = self
            .version
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '.' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        data.join(CACHE).join(format!("{version}-{CUT}"))
    }
}

/// How OpenMap lays the world out in tiles.
struct ZoneArt {
    /// Pixels a world unit has in the tiles.
    px: f64,
    /// A tile's side in pixels, and the part of it that is its own (the rest is its neighbours' edge).
    size: u32,
    step: u32,
    tiles: HashSet<u32>,
}

impl ZoneArt {
    fn parse(text: &str) -> Result<ZoneArt, String> {
        let code: String = text
            .lines()
            .map(|l| l.split("--").next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n");
        let num = |key: &str| -> Result<f64, String> {
            let at = code.find(key).ok_or(format!("ZoneArt.lua has no {key}"))? + key.len();
            let rest = code[at..].trim_start();
            let end = rest
                .find(|c: char| !(c.is_ascii_digit() || c == '.'))
                .unwrap_or(rest.len());
            rest[..end]
                .parse()
                .map_err(|_| format!("ZoneArt.lua: {key} is not a number"))
        };
        let from = code
            .find("tiles")
            .and_then(|i| code[i..].find('{').map(|b| i + b))
            .ok_or("ZoneArt.lua has no tile list")?;
        let to = code[from..]
            .find('}')
            .ok_or("ZoneArt.lua tile list is open")?
            + from;
        let tiles: HashSet<u32> = code[from + 1..to]
            .split(',')
            .filter_map(|t| t.trim().parse().ok())
            .collect();
        let art = ZoneArt {
            px: num("px =")?,
            size: num("size =")? as u32,
            step: num("step =")? as u32,
            tiles,
        };
        if art.px <= 0.0
            || art.size != 512
            || art.step == 0
            || art.step >= art.size
            || art.tiles.is_empty()
        {
            return Err("ZoneArt.lua has a layout this app does not know".into());
        }
        Ok(art)
    }
}

// ---------------------------------------------------------------- BLP

/// A decoded picture: RGBA, 8 bits a channel.
pub struct Image {
    w: usize,
    h: usize,
    px: Vec<u8>,
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// Decodes mip `level` of a BLP2 file compressed with DXT1, DXT3 or DXT5, the only kinds the tiles use.
pub fn decode_blp(file: &[u8], level: u32) -> Result<Image, String> {
    if file.get(..4) != Some(b"BLP2") || u32_at(file, 4) != Some(1) {
        return Err("not a BLP2 file".into());
    }
    let (encoding, alpha_depth, alpha_type) = (file[8], file[9], file[10]);
    if encoding != 2 {
        return Err(format!("BLP encoding {encoding} is not DXT"));
    }
    let w = u32_at(file, 12).ok_or("short BLP")? as usize;
    let h = u32_at(file, 16).ok_or("short BLP")? as usize;
    if w == 0 || h == 0 || w > 4096 || h > 4096 {
        return Err("BLP size out of range".into());
    }
    let level = level as usize;
    let offset = u32_at(file, 20 + 4 * level).ok_or("short BLP")? as usize;
    let size = u32_at(file, 84 + 4 * level).ok_or("short BLP")? as usize;
    let (mw, mh) = ((w >> level).max(1), (h >> level).max(1));
    if level > 15 || size == 0 {
        return Err("BLP has no such mip".into());
    }
    let data = file
        .get(offset..offset + size)
        .ok_or("BLP mip is past the end of the file")?;
    let block = if alpha_depth == 0 { 8 } else { 16 };
    let (bw, bh) = (mw.div_ceil(4), mh.div_ceil(4));
    if data.len() < bw * bh * block {
        return Err("BLP mip is short".into());
    }
    let mut px = vec![0u8; mw * mh * 4];
    for by in 0..bh {
        for bx in 0..bw {
            let b = &data[(by * bw + bx) * block..][..block];
            let (alpha, color) = if block == 16 {
                b.split_at(8)
            } else {
                (&b[..0], b)
            };
            let mut a = [255u8; 16];
            if block == 16 {
                if alpha_type == 7 {
                    let (a0, a1) = (alpha[0] as u32, alpha[1] as u32);
                    let mut pal = [0u8; 8];
                    pal[0] = a0 as u8;
                    pal[1] = a1 as u8;
                    for k in 1..=(if a0 > a1 { 6 } else { 4 }) {
                        pal[k + 1] = if a0 > a1 {
                            (((7 - k as u32) * a0 + k as u32 * a1) / 7) as u8
                        } else {
                            (((5 - k as u32) * a0 + k as u32 * a1) / 5) as u8
                        };
                    }
                    if a0 <= a1 {
                        pal[6] = 0;
                        pal[7] = 255;
                    }
                    let bits = u64::from_le_bytes([
                        alpha[2], alpha[3], alpha[4], alpha[5], alpha[6], alpha[7], 0, 0,
                    ]);
                    for (k, out) in a.iter_mut().enumerate() {
                        *out = pal[((bits >> (3 * k)) & 7) as usize];
                    }
                } else {
                    // DXT3: four bits of alpha a pixel.
                    for (k, out) in a.iter_mut().enumerate() {
                        let nib = (alpha[k / 2] >> (4 * (k % 2))) & 15;
                        *out = nib * 17;
                    }
                }
            }
            let c0 = u16::from_le_bytes([color[0], color[1]]);
            let c1 = u16::from_le_bytes([color[2], color[3]]);
            let rgb = |c: u16| {
                let (r, g, b) = ((c >> 11) & 31, (c >> 5) & 63, c & 31);
                [
                    ((r << 3) | (r >> 2)) as u32,
                    ((g << 2) | (g >> 4)) as u32,
                    ((b << 3) | (b >> 2)) as u32,
                ]
            };
            let (p0, p1) = (rgb(c0), rgb(c1));
            let mut pal = [[0u32; 4]; 4];
            pal[0] = [p0[0], p0[1], p0[2], 255];
            pal[1] = [p1[0], p1[1], p1[2], 255];
            // Only DXT1 (no alpha block) has the three-colour mode with a clear fourth.
            if c0 > c1 || block == 16 {
                for k in 0..3 {
                    pal[2][k] = (2 * p0[k] + p1[k]) / 3;
                    pal[3][k] = (p0[k] + 2 * p1[k]) / 3;
                }
                pal[2][3] = 255;
                pal[3][3] = 255;
            } else {
                for k in 0..3 {
                    pal[2][k] = (p0[k] + p1[k]) / 2;
                }
                pal[2][3] = 255;
            }
            let idx = u32::from_le_bytes([color[4], color[5], color[6], color[7]]);
            for k in 0..16 {
                let (x, y) = (bx * 4 + k % 4, by * 4 + k / 4);
                if x >= mw || y >= mh {
                    continue;
                }
                let c = pal[((idx >> (2 * k)) & 3) as usize];
                let o = (y * mw + x) * 4;
                px[o] = c[0] as u8;
                px[o + 1] = c[1] as u8;
                px[o + 2] = c[2] as u8;
                px[o + 3] = if block == 16 { a[k] } else { c[3] as u8 };
            }
        }
    }
    Ok(Image { w: mw, h: mh, px })
}

// ---------------------------------------------------------------- cutting

/// Tiles decoded so far, so the neighbouring maps of one run do not decode them again.
#[derive(Default)]
pub struct Tiles {
    root: Option<PathBuf>,
    cache: HashMap<(u32, u32), Option<Arc<Image>>>,
}

impl Tiles {
    fn get(
        &mut self,
        inst: &Install,
        art: &ZoneArt,
        i: u32,
        j: u32,
        level: u32,
    ) -> Option<Arc<Image>> {
        if self.root.as_deref() != Some(inst.dir.as_path()) {
            self.root = Some(inst.dir.clone());
            self.cache.clear();
        }
        let key = (i * 256 + j, level);
        if !art.tiles.contains(&key.0) {
            return None;
        }
        if let Some(hit) = self.cache.get(&key) {
            return hit.clone();
        }
        if self.cache.len() >= KEEP {
            self.cache.clear();
        }
        let img = fs::read(inst.tile_path(i, j))
            .ok()
            .and_then(|f| decode_blp(&f, level).ok())
            .map(Arc::new);
        self.cache.insert(key, img.clone());
        img
    }
}

/// `value = a + b * t` for t in 0..1, from the values at 0 and 1.
fn line(at0: f64, at1: f64) -> (f64, f64) {
    (at0, at1 - at0)
}

/// Where a map's sheet lies on the world map: world u and v as lines of the sheet's u and v.
/// Continent rows of Azeroth's own assignment turn yards into the world map's 0..1.
fn placement(e: &Entry) -> Option<((f64, f64), (f64, f64))> {
    let Some(a) = &e.rect else {
        // The world map itself: its own sheet is the world.
        return Some((line(0.0, 1.0), line(0.0, 1.0)));
    };
    let w = table().world.rows.iter().find(|r| r.c == a.c)?;
    // Sheet u runs west to east, which is world y falling; sheet v runs north to south, world x falling.
    let to_u = |u: f64| {
        let wy = a.r[3] - u * (a.r[3] - a.r[1]);
        w.min[0] + (w.r[3] - wy) / (w.r[3] - w.r[1]) * (w.max[0] - w.min[0])
    };
    let to_v = |v: f64| {
        let wx = a.r[2] - v * (a.r[2] - a.r[0]);
        w.min[1] + (w.r[2] - wx) / (w.r[2] - w.r[0]) * (w.max[1] - w.min[1])
    };
    Some((line(to_u(0.0), to_u(1.0)), line(to_v(0.0), to_v(1.0))))
}

/// One map as RGB, `e.w` by `e.h`, bilinear from the tiles (premultiplied, so a tile's clear
/// edge does not bleed) with four taps a pixel, over the sea colour where there is no art.
fn cut_rgb(inst: &Install, art: &ZoneArt, e: &Entry, tiles: &mut Tiles) -> Result<Vec<u8>, String> {
    let ((u0, du), (v0, dv)) =
        placement(e).ok_or("the map is not on a continent the world map shows")?;
    let (w, h) = (e.w as usize, e.h as usize);
    let (ww, wh) = (table().world.w * art.px, table().world.h * art.px);
    // World pixels an output pixel spans: picks the mip, so a continent is not a sieve of single texels.
    let span = (du * ww / w as f64).abs().max((dv * wh / h as f64).abs());
    let level = (span.log2().floor().max(0.0) as u32).min(MAX_MIP);
    let (step, scale) = (art.step as f64, 2f64.powi(level as i32));
    let mut out = vec![0u8; w * h * 3];
    for oy in 0..h {
        for ox in 0..w {
            let (mut sum, mut cover) = ([0f32; 3], 0f32);
            for (sy, sx) in [(0.25, 0.25), (0.25, 0.75), (0.75, 0.25), (0.75, 0.75)] {
                let x = (u0 + du * (ox as f64 + sx) / w as f64) * ww;
                let y = (v0 + dv * (oy as f64 + sy) / h as f64) * wh;
                if x < 0.0 || y < 0.0 || x >= ww || y >= wh {
                    continue;
                }
                let (i, j) = ((x / step) as u32, (y / step) as u32);
                let Some(t) = tiles.get(inst, art, i, j, level) else {
                    continue;
                };
                // The tile's first pixel is its neighbour's edge, so its own start at 1.
                let cx = ((x - i as f64 * step + 1.0) / scale - 0.5).max(0.0);
                let cy = ((y - j as f64 * step + 1.0) / scale - 0.5).max(0.0);
                let (x0, y0) = (cx as usize, cy as usize);
                let (fx, fy) = ((cx - x0 as f64) as f32, (cy - y0 as f64) as f32);
                let (x1, y1) = ((x0 + 1).min(t.w - 1), (y0 + 1).min(t.h - 1));
                let (x0, y0) = (x0.min(t.w - 1), y0.min(t.h - 1));
                for (px, wgt) in [
                    ((x0, y0), (1.0 - fx) * (1.0 - fy)),
                    ((x1, y0), fx * (1.0 - fy)),
                    ((x0, y1), (1.0 - fx) * fy),
                    ((x1, y1), fx * fy),
                ] {
                    let p = &t.px[(px.1 * t.w + px.0) * 4..][..4];
                    let a = p[3] as f32 / 255.0 * wgt;
                    for k in 0..3 {
                        sum[k] += p[k] as f32 * a;
                    }
                    cover += a;
                }
            }
            let o = (oy * w + ox) * 3;
            for k in 0..3 {
                out[o + k] = ((sum[k] + SEA[k] * (4.0 - cover)) / 4.0)
                    .round()
                    .clamp(0.0, 255.0) as u8;
            }
        }
    }
    Ok(out)
}

fn encode(rgb: &[u8], w: u32, h: u32) -> Vec<u8> {
    webp::Encoder::from_rgb(rgb, w, h)
        .encode(WEBP_QUALITY)
        .to_vec()
}

fn tiles() -> &'static Mutex<Tiles> {
    static T: std::sync::OnceLock<Mutex<Tiles>> = std::sync::OnceLock::new();
    T.get_or_init(|| Mutex::new(Tiles::default()))
}

/// The picture of one map as WebP, from the cache when it is there, else cut now and cached.
pub fn image(inst: &Install, data: &Path, id: u32) -> Option<Vec<u8>> {
    let e = maps().iter().find(|m| m.id == id && !m.city)?;
    let dir = inst.cache_dir(data);
    let file = dir.join(format!("{id}.webp"));
    if let Ok(bytes) = fs::read(&file) {
        return Some(bytes);
    }
    let art = inst.zone_art().ok()?;
    let rgb = {
        let mut t = tiles().lock().unwrap_or_else(|p| p.into_inner());
        cut_rgb(inst, &art, e, &mut t).ok()?
    };
    let bytes = encode(&rgb, e.w, e.h);
    store(&dir, &file, &bytes);
    Some(bytes)
}

/// Writes a cached picture without leaving half a file, and removes the pictures of older versions.
fn store(dir: &Path, file: &Path, bytes: &[u8]) {
    if fs::create_dir_all(dir).is_err() {
        return;
    }
    let tmp = file.with_extension("tmp");
    if fs::write(&tmp, bytes).is_ok() && fs::rename(&tmp, file).is_err() {
        let _ = fs::remove_file(&tmp);
    }
    prune(dir);
}

/// Deletes the other versions' folders next to `keep`: `<version>-<number>` folders holding only
/// pictures this module wrote.
fn prune(keep: &Path) {
    let Some(parent) = keep.parent() else { return };
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        let ours = p.is_dir()
            && p != keep
            && name
                .rsplit_once('-')
                .is_some_and(|(_, n)| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
        if !ours {
            continue;
        }
        let files: Vec<_> = fs::read_dir(&p)
            .map(|d| d.flatten().collect())
            .unwrap_or_default();
        if files.iter().all(|f| {
            f.path()
                .extension()
                .is_some_and(|x| x == "webp" || x == "tmp")
        }) {
            for f in files {
                let _ = fs::remove_file(f.path());
            }
            let _ = fs::remove_dir(&p);
        }
    }
}

/// Cuts every map into `dir` (the cache's, or another folder), reporting each map's id and bytes.
/// For measuring the cost of a full cache and for tools that need the pictures on disk.
pub fn build_all(
    inst: &Install,
    dir: &Path,
    mut each: impl FnMut(u32, usize),
) -> Result<(), String> {
    let art = inst.zone_art()?;
    fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut t = Tiles::default();
    for e in maps().iter().filter(|m| !m.city) {
        let rgb = match cut_rgb(inst, &art, e, &mut t) {
            Ok(rgb) => rgb,
            Err(_) => continue,
        };
        let bytes = encode(&rgb, e.w, e.h);
        let file = dir.join(format!("{}.webp", e.id));
        let tmp = file.with_extension("tmp");
        fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
        fs::rename(&tmp, &file).map_err(|e| e.to_string())?;
        each(e.id, bytes.len());
    }
    Ok(())
}

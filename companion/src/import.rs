//! History brought in from other addons' saved variables: TradeSkillMaster's accounting and gold
//! log, and Auctioneer's BeanCounter. It is a third kind of record beside lifetime and observed
//! numbers (docs/FORMAT.md, "Imported history"): never counted as Scarlet-observed, never added
//! to the overview's earned and spent, and dropped where Scarlet recorded the same thing itself.

use crate::archive::Event;
use crate::lua::{self, Lua};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

pub const SOURCE_TSM: &str = "TSM";
pub const SOURCE_AUCTIONEER: &str = "Auctioneer";

const DAY_MS: i64 = 86_400_000;
const AUCTION_WINDOW_MS: i64 = 48 * 3_600_000;
const VENDOR_WINDOW_MS: i64 = 60_000;
const MONEY_WINDOW_MS: i64 = 90_000;
const GOLD_WINDOW_MS: i64 = 15 * 60_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Gold,
    Sale,
    Buy,
    Other,
}

/// One record read from another addon, before it is checked against Scarlet's own lines.
#[derive(Clone, Debug)]
pub struct Rec {
    pub player: String,
    pub ms: i64,
    pub kind: Kind,
    pub source: &'static str,
    /// `auction`, `vendor`, `trade` or `other`; for `Other` the kind of income or expense.
    pub via: String,
    pub item: Option<i64>,
    pub qty: i64,
    /// Gold points: the balance. Sales and buys: the whole amount. Other: signed.
    pub copper: i64,
}

fn name_key(name: &str) -> String {
    name.split(|c: char| c.is_whitespace() || c == '-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-")
        .to_lowercase()
}

fn num(s: &str) -> Option<i64> {
    s.trim().parse::<i64>().ok()
}

fn item_id(s: &str) -> Option<i64> {
    let rest = s.strip_prefix("i:").or_else(|| s.strip_prefix("item:"))?;
    num(rest.split(':').next()?)
}

fn csv_rows(text: &str) -> impl Iterator<Item = Vec<&str>> {
    text.lines()
        .skip(1)
        .filter(|l| !l.is_empty())
        .map(|l| l.split(',').collect())
}

fn via(source: &str) -> String {
    match source.to_lowercase().as_str() {
        "auction" => "auction".into(),
        "vendor" => "vendor".into(),
        "trade" => "trade".into(),
        _ => "other".into(),
    }
}

/// Every TradeSkillMaster and BeanCounter saved-variables file of these game folders, one per
/// game account.
pub fn find_files(game: &Path) -> Vec<(PathBuf, &'static str)> {
    let mut out = Vec::new();
    let accounts = game.join("WTF").join("Account");
    let Ok(entries) = fs::read_dir(&accounts) else {
        return out;
    };
    for acc in entries.flatten() {
        let sv = acc.path().join("SavedVariables");
        for (file, source) in [
            ("TradeSkillMaster.lua", SOURCE_TSM),
            ("BeanCounter.lua", SOURCE_AUCTIONEER),
        ] {
            let p = sv.join(file);
            if p.is_file() {
                out.push((p, source));
            }
        }
    }
    out.sort();
    out
}

/// TradeSkillMaster keeps its data in one table of flat keys such as
/// `r@Normal@internalData@csvSales` (CSV text for the whole realm) and
/// `s@Name Surname - Horde - Normal@internalData@goldLog` (one character's balance).
pub fn read_tsm(text: &str) -> Result<Vec<Rec>, String> {
    let Some(db) = lua::parse_named(text, "TradeSkillMasterDB")? else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for (key, value) in db.items().unwrap_or(&[]) {
        let Some(body) = value.as_str() else { continue };
        let parts: Vec<&str> = key.split('@').collect();
        match parts.as_slice() {
            ["s", scope, "internalData", "goldLog"] => {
                let Some(player) = scope.split(" - ").next() else {
                    continue;
                };
                for row in csv_rows(body) {
                    let (Some(minute), Some(copper)) = (
                        row.first().and_then(|v| num(v)),
                        row.get(1).and_then(|v| num(v)),
                    ) else {
                        continue;
                    };
                    out.push(Rec {
                        player: player.to_string(),
                        ms: minute * 60_000,
                        kind: Kind::Gold,
                        source: SOURCE_TSM,
                        via: String::new(),
                        item: None,
                        qty: 0,
                        copper,
                    });
                }
            }
            ["r", _, "internalData", "csvSales"] | ["r", _, "internalData", "csvBuys"] => {
                let kind = if parts[3] == "csvSales" {
                    Kind::Sale
                } else {
                    Kind::Buy
                };
                for row in csv_rows(body) {
                    let (Some(id), Some(qty), Some(price), Some(time)) = (
                        row.first().and_then(|v| item_id(v)),
                        row.get(2).and_then(|v| num(v)),
                        row.get(3).and_then(|v| num(v)),
                        row.get(6).and_then(|v| num(v)),
                    ) else {
                        continue;
                    };
                    out.push(Rec {
                        player: row.get(5).copied().unwrap_or("").to_string(),
                        ms: time * 1000,
                        kind,
                        source: SOURCE_TSM,
                        via: via(row.get(7).copied().unwrap_or("")),
                        item: Some(id),
                        qty,
                        copper: price * qty,
                    });
                }
            }
            ["r", _, "internalData", "csvIncome"] | ["r", _, "internalData", "csvExpense"] => {
                let sign = if parts[3] == "csvIncome" { 1 } else { -1 };
                for row in csv_rows(body) {
                    let (Some(amount), Some(time)) = (
                        row.get(1).and_then(|v| num(v)),
                        row.get(4).and_then(|v| num(v)),
                    ) else {
                        continue;
                    };
                    out.push(Rec {
                        player: row.get(3).copied().unwrap_or("").to_string(),
                        ms: time * 1000,
                        kind: Kind::Other,
                        source: SOURCE_TSM,
                        via: row
                            .first()
                            .copied()
                            .unwrap_or("")
                            .to_lowercase()
                            .replace(' ', "-"),
                        item: None,
                        qty: 0,
                        copper: sign * amount,
                    });
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

fn bean_key_id(key: &str) -> Option<i64> {
    if let Some(p) = key.find("item:") {
        return num(key[p + 5..].split(|c: char| !c.is_ascii_digit()).next()?);
    }
    num(key)
}

fn bean_walk(
    v: &Lua,
    id: Option<i64>,
    player: &str,
    kind: Kind,
    out: &mut Vec<Rec>,
    skipped: &mut usize,
) {
    let Some(items) = v.items() else { return };
    for (key, child) in items {
        let id = id.or_else(|| bean_key_id(key));
        match child {
            Lua::Table(_) => bean_walk(child, id, player, kind, out, skipped),
            Lua::Str(s) => match (bean_entry(s), id) {
                (Some((stack, money, time)), Some(id)) => out.push(Rec {
                    player: player.to_string(),
                    ms: time * 1000,
                    kind,
                    source: SOURCE_AUCTIONEER,
                    via: "auction".into(),
                    item: Some(id),
                    qty: stack,
                    copper: money,
                }),
                _ => *skipped += 1,
            },
            _ => {}
        }
    }
}

fn bean_entry(s: &str) -> Option<(i64, i64, i64)> {
    let f: Vec<&str> = s.split(';').collect();
    let stack = num(f.first()?)?;
    if stack > 100_000 {
        return None;
    }
    let money = num(f.get(1)?)?.abs();
    let time = num(f.get(7)?)?;
    (time > 1_000_000_000 && stack > 0).then_some((stack, money, time))
}

/// BeanCounter: `BeanCounterDB[realm][character]`, tables `completedAuctions` (sales) and
/// `completedBidsBuyouts` or `completedBids/Buyouts` (purchases), each grouped by item. Returns
/// the records and how many entries had a layout that was not read.
pub fn read_beancounter(text: &str) -> Result<(Vec<Rec>, usize), String> {
    let Some(db) = lua::parse_named(text, "BeanCounterDB")? else {
        return Ok((Vec::new(), 0));
    };
    let mut out = Vec::new();
    let mut skipped = 0;
    for (_, realm) in db.items().unwrap_or(&[]) {
        for (player, tables) in realm.items().unwrap_or(&[]) {
            for (table, kind) in [
                ("completedAuctions", Kind::Sale),
                ("completedBidsBuyouts", Kind::Buy),
                ("completedBids/Buyouts", Kind::Buy),
            ] {
                if let Some(t) = tables.get(table) {
                    bean_walk(t, None, player, kind, &mut out, &mut skipped);
                }
            }
        }
    }
    Ok((out, skipped))
}

/// Records dropped because Scarlet has the same thing from its own lines.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
#[serde(default)]
pub struct Dropped {
    pub sales: i64,
    pub buys: i64,
    pub other: i64,
    pub gold: i64,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct GoldPt {
    pub t: i64,
    pub m: i64,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct Row {
    pub t: i64,
    /// `s` sale, `b` purchase.
    pub k: String,
    pub id: i64,
    pub name: String,
    pub q: i64,
    pub c: i64,
    pub via: String,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct ItemPrice {
    pub id: i64,
    pub name: String,
    pub sold: i64,
    pub sold_copper: i64,
    /// Lowest and highest price of one item sold.
    pub low: i64,
    pub high: i64,
    pub bought: i64,
    pub bought_copper: i64,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct Day {
    pub d: i64,
    pub sold: i64,
    pub sold_copper: i64,
    pub bought: i64,
    pub bought_copper: i64,
}

/// What the app, the web page and the in-game pages show of one character's imported history.
/// Every number counts only records Scarlet did not record itself.
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct View {
    pub sources: Vec<String>,
    pub first: i64,
    pub last: i64,
    pub sales: i64,
    pub sales_copper: i64,
    pub buys: i64,
    pub buys_copper: i64,
    pub dropped: Dropped,
    /// The last balance of each UTC day, with its time, oldest first.
    pub gold: Vec<GoldPt>,
    /// Newest first.
    pub recent: Vec<Row>,
    pub items: Vec<ItemPrice>,
    /// Newest first.
    pub days: Vec<Day>,
}

pub struct Built {
    pub kept: Vec<Rec>,
    pub view: View,
}

struct Native<'a> {
    names: HashMap<i64, &'a str>,
    by_name: HashMap<&'a str, Vec<i64>>,
    av_sell: Vec<(i64, &'a str, i64, i64, bool)>,
    av_buy: Vec<(i64, &'a str, i64, i64, bool)>,
    au_buy: Vec<(i64, i64, i64, i64, bool)>,
    vend_sell: Vec<(i64, i64, i64, bool)>,
    vend_buy: Vec<(i64, i64, i64, bool)>,
    money: Vec<(i64, i64, bool)>,
    anchors: Vec<i64>,
}

fn pick_lines(counts: &[i64], want: i64) -> Option<Vec<usize>> {
    let mut sum = 0;
    for (k, c) in counts.iter().enumerate() {
        sum += c;
        if sum == want {
            return Some((0..=k).collect());
        }
        if sum > want {
            break;
        }
    }
    subset_sum(&counts[..counts.len().min(14)], want)
}

fn subset_sum(counts: &[i64], want: i64) -> Option<Vec<usize>> {
    fn go(counts: &[i64], from: usize, left: i64, picked: &mut Vec<usize>) -> bool {
        if left == 0 {
            return !picked.is_empty();
        }
        for k in from..counts.len() {
            if counts[k] > 0 && counts[k] <= left {
                picked.push(k);
                if go(counts, k + 1, left - counts[k], picked) {
                    return true;
                }
                picked.pop();
            }
        }
        false
    }
    let mut picked = Vec::new();
    go(counts, 0, want, &mut picked).then_some(picked)
}

fn f(ev: &Event, i: usize) -> &str {
    ev.fields.get(i).map_or("", String::as_str)
}

fn n(ev: &Event, i: usize) -> Option<i64> {
    ev.fields.get(i).and_then(|s| s.trim().parse().ok())
}

impl<'a> Native<'a> {
    fn new(evs: &'a [Event]) -> Self {
        let mut s = Native {
            names: HashMap::new(),
            by_name: HashMap::new(),
            av_sell: Vec::new(),
            av_buy: Vec::new(),
            au_buy: Vec::new(),
            vend_sell: Vec::new(),
            vend_buy: Vec::new(),
            money: Vec::new(),
            anchors: Vec::new(),
        };
        for ev in evs {
            match ev.code.as_str() {
                "IN" => {
                    if let Some(id) = n(ev, 0) {
                        s.names.insert(id, f(ev, 1));
                        s.by_name.entry(f(ev, 1)).or_default().push(id);
                    }
                }
                "AV" => {
                    let (count, bid, cut) = (
                        n(ev, 2).unwrap_or(0),
                        n(ev, 3).unwrap_or(0),
                        n(ev, 6).unwrap_or(0),
                    );
                    match f(ev, 0) {
                        "seller" => s.av_sell.push((ev.ms, f(ev, 1), count, bid - cut, false)),
                        "buyer" => s.av_buy.push((ev.ms, f(ev, 1), count, bid, false)),
                        _ => {}
                    }
                }
                "AU" if f(ev, 0) == "y" => {
                    if let (Some(id), Some(count), Some(paid)) = (n(ev, 2), n(ev, 3), n(ev, 4)) {
                        s.au_buy.push((ev.ms, id, count, paid, false));
                    }
                }
                "I" if f(ev, 2) == "v" => {
                    if let (Some(id), Some(d)) = (n(ev, 0), n(ev, 1))
                        && d < 0
                    {
                        s.vend_sell.push((ev.ms, id, -d, false));
                    }
                }
                "VB" if f(ev, 0) == "b" => {
                    if let (Some(id), Some(count)) = (n(ev, 1), n(ev, 2)) {
                        s.vend_buy.push((ev.ms, id, count, false));
                    }
                }
                "M" => {
                    if let Some(d) = n(ev, 0) {
                        s.money.push((ev.ms, d, false));
                    }
                    if n(ev, 2).is_some() {
                        s.anchors.push(ev.ms);
                    }
                }
                "S" if n(ev, 12).is_some() => s.anchors.push(ev.ms),
                _ => {}
            }
        }
        s.anchors.sort_unstable();
        s
    }

    fn name_of(&self, id: i64) -> &str {
        self.names.get(&id).copied().unwrap_or("")
    }

    fn take_invoice(&mut self, sell: bool, r: &Rec) -> bool {
        let id = r.item.unwrap_or(0);
        let name = self.names.get(&id).copied().unwrap_or("");
        if name.is_empty() {
            return false;
        }
        let tol = r.qty.max(2);
        let list = if sell {
            &mut self.av_sell
        } else {
            &mut self.av_buy
        };
        let hit = list
            .iter_mut()
            .filter(|(t, nm, q, amt, used)| {
                !*used
                    && *nm == name
                    && *q == r.qty
                    && (amt - r.copper).abs() <= tol
                    && (t - r.ms).abs() <= AUCTION_WINDOW_MS
            })
            .min_by_key(|(t, ..)| (t - r.ms).abs());
        match hit {
            Some(h) => {
                h.4 = true;
                true
            }
            None => false,
        }
    }

    fn take_auction_buy(&mut self, r: &Rec) -> bool {
        let id = r.item.unwrap_or(0);
        let tol = r.qty.max(2);
        let hit = self
            .au_buy
            .iter_mut()
            .filter(|(t, i, q, paid, used)| {
                !*used
                    && *i == id
                    && *q == r.qty
                    && (paid - r.copper).abs() <= tol
                    && (t - r.ms).abs() <= AUCTION_WINDOW_MS
            })
            .min_by_key(|(t, ..)| (t - r.ms).abs());
        match hit {
            Some(h) => {
                h.4 = true;
                true
            }
            None => false,
        }
    }

    fn take_vendor(&mut self, sell: bool, r: &Rec) -> bool {
        let id = r.item.unwrap_or(0);
        let list = if sell {
            &mut self.vend_sell
        } else {
            &mut self.vend_buy
        };
        let mut near: Vec<usize> = (0..list.len())
            .filter(|&i| {
                !list[i].3 && list[i].1 == id && (list[i].0 - r.ms).abs() <= VENDOR_WINDOW_MS
            })
            .collect();
        near.sort_by_key(|&i| (list[i].0 - r.ms).abs());
        let counts: Vec<i64> = near.iter().map(|&i| list[i].2).collect();
        let Some(pick) = pick_lines(&counts, r.qty) else {
            return false;
        };
        let taken: Vec<usize> = pick.into_iter().map(|k| near[k]).collect();
        for i in taken {
            list[i].3 = true;
        }
        true
    }

    fn take_money(&mut self, r: &Rec) -> bool {
        let hit = self
            .money
            .iter_mut()
            .filter(|(t, d, used)| {
                !*used && (d - r.copper).abs() <= 1 && (t - r.ms).abs() <= MONEY_WINDOW_MS
            })
            .min_by_key(|(t, ..)| (t - r.ms).abs());
        match hit {
            Some(h) => {
                h.2 = true;
                true
            }
            None => false,
        }
    }

    fn covers_gold(&self, ms: i64) -> bool {
        let i = self.anchors.partition_point(|t| *t < ms);
        [i.checked_sub(1), Some(i)]
            .into_iter()
            .flatten()
            .filter_map(|j| self.anchors.get(j))
            .any(|t| (t - ms).abs() <= GOLD_WINDOW_MS)
    }
}

/// The records of one character that Scarlet does not have from its own lines, and the view of them.
/// `evs` is every line Scarlet has for the character, whether recorded by the addon or read in
/// earlier, in time order.
pub fn build(recs: Vec<Rec>, evs: &[Event]) -> Built {
    let mut native = Native::new(evs);
    let mut recs = recs;
    recs.sort_by_key(|r| (r.ms, r.copper));
    let mut dropped = Dropped::default();
    let mut kept = Vec::new();
    for r in recs {
        let dup = match (r.kind, r.via.as_str()) {
            (Kind::Gold, _) => {
                let d = native.covers_gold(r.ms);
                if d {
                    dropped.gold += 1;
                }
                d
            }
            (Kind::Sale, "auction") => native.take_invoice(true, &r),
            (Kind::Buy, "auction") => native.take_invoice(false, &r) || native.take_auction_buy(&r),
            (Kind::Sale, "vendor") => native.take_vendor(true, &r),
            (Kind::Buy, "vendor") => native.take_vendor(false, &r),
            (Kind::Other, _) => native.take_money(&r),
            _ => false,
        };
        if dup {
            match r.kind {
                Kind::Sale => dropped.sales += 1,
                Kind::Buy => dropped.buys += 1,
                Kind::Other => dropped.other += 1,
                Kind::Gold => {}
            }
            continue;
        }
        kept.push(r);
    }
    let view = view_of(&kept, dropped, &native);
    Built { kept, view }
}

fn view_of(kept: &[Rec], dropped: Dropped, native: &Native) -> View {
    let mut v = View {
        dropped,
        ..View::default()
    };
    let mut sources: Vec<String> = Vec::new();
    let mut gold: BTreeMap<i64, (i64, i64)> = BTreeMap::new();
    let mut days: BTreeMap<i64, Day> = BTreeMap::new();
    let mut items: BTreeMap<i64, ItemPrice> = BTreeMap::new();
    let mut trades: Vec<&Rec> = Vec::new();
    for r in kept {
        if !sources.iter().any(|s| s == r.source) {
            sources.push(r.source.to_string());
        }
        if v.first == 0 || r.ms < v.first {
            v.first = r.ms;
        }
        v.last = v.last.max(r.ms);
        match r.kind {
            Kind::Gold => {
                gold.insert(r.ms.div_euclid(DAY_MS), (r.ms.div_euclid(1000), r.copper));
            }
            Kind::Sale | Kind::Buy => {
                let sale = r.kind == Kind::Sale;
                let day = days.entry(r.ms.div_euclid(DAY_MS)).or_default();
                day.d = r.ms.div_euclid(DAY_MS) * 86_400;
                let id = r.item.unwrap_or(0);
                let it = items.entry(id).or_insert_with(|| ItemPrice {
                    id,
                    name: native.name_of(id).to_string(),
                    low: i64::MAX,
                    ..ItemPrice::default()
                });
                if sale {
                    v.sales += 1;
                    v.sales_copper += r.copper;
                    day.sold += r.qty;
                    day.sold_copper += r.copper;
                    it.sold += r.qty;
                    it.sold_copper += r.copper;
                    let unit = r.copper / r.qty.max(1);
                    it.low = it.low.min(unit);
                    it.high = it.high.max(unit);
                } else {
                    v.buys += 1;
                    v.buys_copper += r.copper;
                    day.bought += r.qty;
                    day.bought_copper += r.copper;
                    it.bought += r.qty;
                    it.bought_copper += r.copper;
                }
                trades.push(r);
            }
            Kind::Other => {}
        }
    }
    sources.sort();
    v.sources = sources;
    v.gold = gold.into_values().map(|(t, m)| GoldPt { t, m }).collect();
    trades.sort_by_key(|r| std::cmp::Reverse(r.ms));
    v.recent = trades
        .iter()
        .take(40)
        .map(|r| Row {
            t: r.ms.div_euclid(1000),
            k: if r.kind == Kind::Sale { "s" } else { "b" }.into(),
            id: r.item.unwrap_or(0),
            name: native.name_of(r.item.unwrap_or(0)).to_string(),
            q: r.qty,
            c: r.copper,
            via: r.via.clone(),
        })
        .collect();
    let mut list: Vec<ItemPrice> = items
        .into_values()
        .map(|mut i| {
            if i.low == i64::MAX {
                i.low = 0;
            }
            i
        })
        .collect();
    list.sort_by_key(|i| std::cmp::Reverse(i.sold_copper + i.bought_copper));
    list.truncate(60);
    v.items = list;
    v.days = days.into_values().rev().take(90).collect();
    v
}

/// The payload text: `scarletimports<TAB>1<TAB>Name-Surname`, then `<epoch ms><TAB><code><TAB><fields>`:
/// `YG source balance`, `YS source via item quantity copper`, `YB` the same for purchases,
/// `YM source kind signedCopper`.
pub fn scl_text(name: &str, kept: &[Rec]) -> String {
    let mut out = format!("scarletimports\t1\t{name}\n");
    for r in kept {
        let line = match r.kind {
            Kind::Gold => format!("{}\tYG\t{}\t{}", r.ms, r.source, r.copper),
            Kind::Sale | Kind::Buy => format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}",
                r.ms,
                if r.kind == Kind::Sale { "YS" } else { "YB" },
                r.source,
                r.via,
                r.item.unwrap_or(0),
                r.qty,
                r.copper
            ),
            Kind::Other => format!("{}\tYM\t{}\t{}\t{}", r.ms, r.source, r.via, r.copper),
        };
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// Write one character's imports: `imports/payloads/import.scl` (what is uploaded) and
/// `imports/view.json` (what the pages show). With nothing to keep both are removed.
pub fn write(char_dir: &Path, name: &str, built: &Built) -> Result<(), Box<dyn std::error::Error>> {
    let base = char_dir.join("imports");
    let scl = base.join("payloads").join("import.scl");
    if built.kept.is_empty() && built.view.dropped == Dropped::default() {
        let _ = fs::remove_file(&scl);
        let _ = fs::remove_file(base.join("view.json"));
        return Ok(());
    }
    fs::create_dir_all(base.join("payloads"))?;
    let payload = zstd::stream::encode_all(scl_text(name, &built.kept).as_bytes(), 19)?;
    let tmp = scl.with_extension("scl.tmp");
    fs::write(&tmp, payload)?;
    fs::rename(&tmp, &scl)?;
    let tmp = base.join("view.json.tmp");
    fs::write(&tmp, serde_json::to_string(&built.view)?)?;
    fs::rename(&tmp, base.join("view.json"))?;
    Ok(())
}

/// The view `write` saved for this character, if any.
pub fn read_view(char_dir: &Path) -> Option<View> {
    let text = fs::read_to_string(char_dir.join("imports").join("view.json")).ok()?;
    serde_json::from_str(&text).ok()
}

/// The records of every source file for the characters in `own`, by Name-Surname.
pub fn read_all(
    game_dirs: &[PathBuf],
    own: &[String],
) -> (BTreeMap<String, Vec<Rec>>, Vec<String>) {
    let mut by: BTreeMap<String, Vec<Rec>> = BTreeMap::new();
    let mut notes = Vec::new();
    for game in game_dirs {
        for (path, source) in find_files(game) {
            let Ok(text) = fs::read_to_string(&path) else {
                notes.push(format!("{}: unreadable", path.display()));
                continue;
            };
            let recs = if source == SOURCE_TSM {
                read_tsm(&text)
            } else {
                read_beancounter(&text).map(|(r, skipped)| {
                    if skipped > 0 {
                        notes.push(format!(
                            "{}: {skipped} entries in a layout not read",
                            path.display()
                        ));
                    }
                    r
                })
            };
            match recs {
                Ok(recs) => {
                    for r in recs {
                        let key = name_key(&r.player);
                        if let Some(me) = own.iter().find(|o| name_key(o) == key) {
                            by.entry(me.clone()).or_default().push(r);
                        }
                    }
                }
                Err(e) => notes.push(format!("{}: {e}", path.display())),
            }
        }
    }
    (by, notes)
}

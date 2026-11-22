//! Chunk text -> events, and the two renderings (readable .txt lines and
//! raw .scl lines).

use crate::tables::{plain_value, stat_number};
use chrono::{Local, TimeZone};
use std::collections::HashMap;

/// One event. `cs` is the absolute time in epoch centiseconds
/// (chunk time base + sum of dt up to and including this line).
pub struct Ev {
    pub cs: i64,
    pub code: String,
    pub fields: Vec<String>,
}

/// Parse the decoded chunk text. `base` is the chunk's time base `t` in
/// epoch seconds; the first line's dt counts from it.
pub fn parse_chunk_text(text: &str, base: i64) -> Vec<Ev> {
    let mut cum: i64 = 0;
    let mut out = Vec::new();
    for line in text.split('\n') {
        if line.is_empty() {
            continue;
        }
        let mut it = line.split('\t');
        let dt = it.next().and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
        cum += dt;
        let code = it.next().unwrap_or("").to_string();
        let fields: Vec<String> = it.map(str::to_string).collect();
        out.push(Ev {
            cs: base * 100 + cum,
            code,
            fields,
        });
    }
    out
}

pub fn word(code: &str) -> &str {
    match code {
        "S" => "session",
        "ES" => "end",
        "Z" => "zone",
        "P" => "path",
        "W" => "moved",
        "T" => "flight",
        "C" => "cast",
        "K" => "kill",
        "N" => "name",
        "D" => "died",
        "R" => "released",
        "U" => "revived",
        "F" => "combat",
        "E" => "encounter",
        "B" => "boss",
        "L" => "level",
        "X" => "xp",
        "M" => "money",
        "O" => "loot",
        "Q" => "quest",
        "G" => "gear",
        "SK" => "skill",
        "RP" => "rep",
        "AC" => "achieve",
        "LG" => "combatlog",
        "A" => "aura",
        "CP" => "camp",
        "RS" => "resting",
        "I" => "item",
        "SN" => "spellname",
        "IN" => "itemname",
        "CH" => "channel",
        "CI" => "interrupt",
        "AFK" => "afk",
        "RX" => "rested",
        "RO" => "restedlogout",
        "GP" => "group",
        "GM" => "member",
        "WD" => "said",
        "EM" => "emote",
        "DM" => "meter",
        "DS" => "topspell",
        "DP" => "groupmeter",
        "DL" => "deathspot",
        "RZ" => "rezoffer",
        "SH" => "spirithealer",
        "DU" => "duelask",
        "DW" => "duel",
        "EX" => "discover",
        "XR" => "restedxp",
        "MT" => "timer",
        "DB" => "durability",
        "RE" => "repair",
        "NI" => "npc",
        "QN" => "questname",
        "LS" => "learned",
        "LR" => "recipe",
        "TL" => "talents",
        "HB" => "hearth",
        "HK" => "hk",
        "HN" => "honor",
        "PV" => "pvp",
        "PT" => "pet",
        "SS" => "screenshot",
        "TP" => "played",
        "RL" => "roll",
        "ST" => "stats",
        "GR" => "rules",
        "DR" => "deathhit",
        "CU" => "currency",
        "CN" => "currencyname",
        "CR" => "crafted",
        "CC" => "cc",
        "NC" => "collected",
        "TS" => "profession",
        "CB" => "craft",
        "CS" => "craftstop",
        "CW" => "craftpaid",
        "SC" => "stat",
        "SL" => "skillline",
        "KN" => "skillname",
        "WS" => "statistic",
        "WN" => "statname",
        "AP" => "achpoints",
        "AN" => "achname",
        "AE" => "criterion",
        "LV" => "renown",
        "LB" => "legacybal",
        "LA" => "legacyach",
        "LN" => "legacynode",
        "LO" => "lootsrc",
        "LW" => "peerloot",
        "PR" => "api",
        "TW" => "totem",
        "IL" => "lock",
        "SKN" => "skinned",
        "SR" => "selfres",
        "RD" => "recipelevel",
        "RT" => "tracked",
        "PG" => "profgear",
        "PS" => "specpoints",
        "PU" => "specunlock",
        "PO" => "orders",
        "CO" => "orderdone",
        "QI" => "questinfo",
        "QP" => "objective",
        "QS" => "queststate",
        "QA" => "questshared",
        "QD" => "questdialog",
        "QR" => "questreward",
        "QW" => "questwatch",
        "QY" => "questsync",
        "QK" => "campaign",
        "NW" => "window",
        "GS" => "gossip",
        "GO" => "gossippick",
        "GN" => "gossipname",
        "GC" => "gossipconfirm",
        "PC" => "choice",
        "IR" => "read",
        "CK" => "contenttrack",
        "VB" => "vendorbuy",
        "VS" => "vendorstock",
        "AU" => "auction",
        "ML" => "mail",
        "TR" => "trade",
        "BK" => "bank",
        "BG" => "bagspace",
        "EN" => "enchant",
        "WE" => "tempench",
        "IU" => "upgrade",
        "RW" => "lootroll",
        "BR" => "bonusroll",
        "TK" => "token",
        "LP" => "lootspec",
        "DI" => "deleted",
        "RF" => "standing",
        "FN" => "factname",
        "RG" => "paragon",
        "RV" => "majorrep",
        "RR" => "renownrew",
        "AD" => "achdetail",
        "AT" => "achtrack",
        "BQ" => "bgqueue",
        "BM" => "match",
        "BS" => "scoreboard",
        "BN" => "statname",
        "HL" => "honorlevel",
        "PB" => "pvpstate",
        "ZP" => "zonepvp",
        "LM" => "lootrule",
        "LT" => "grouploot",
        "LD" => "grouproll",
        "IQ" => "itemquality",
        "II" => "iteminfo",
        "IT" => "itemtip",
        "TN" => "flightpoint",
        "TF" => "flightpick",
        "MO" => "mount",
        "WP" => "waypoint",
        "SM" => "summon",
        "TE" => "teleport",
        "NP" => "place",
        "TM" => "countdown",
        "IB" => "instanceboot",
        "DF" => "difficulty",
        "VG" => "vignette",
        "AI" => "areapoi",
        "MP" => "tracking",
        "SE" => "scenario",
        "TG" => "spec",
        "TA" => "talent",
        "TY" => "talentname",
        "TV" => "talentlayout",
        "TD" => "talentnode",
        "TQ" => "respec",
        "TI" => "title",
        "EQ" => "equipset",
        "PD" => "petinfo",
        "XD" => "xplock",
        "BA" => "barber",
        "GA" => "gamemode",
        "CD" => "identity",
        "SX" => "stat",
        "SB" => "spellbook",
        "AB" => "actionbar",
        "TB" => "trainerbuy",
        "TC" => "trainerlist",
        "SG" => "stable",
        "AR" => "autorepeat",
        "SO" => "proc",
        "EC" => "enchanted",
        "CL" => "collection",
        "CA" => "added",
        "CX" => "removed",
        "SP" => "companion",
        "TX" => "transmog",
        "TO" => "outfit",
        "BT" => "petbattle",
        "GU" => "guild",
        "GH" => "guildsize",
        "FR" => "friend",
        "FC" => "friendcount",
        "GI" => "invite",
        "GL" => "groupstate",
        "RC" => "readycheck",
        "LF" => "finder",
        "IV" => "inspect",
        "NS" => "creature",
        "PM" => "player",
        "PK" => "playerkill",
        "AV" => "invoice",
        "AL" => "ally",
        "CT" => "chat",
        "DQ" => "duelinfo",
        "CE" => "calendar",
        "CM" => "community",
        "CV" => "cinematic",
        "LL" => "loading",
        "LK" => "logout",
        "UE" => "uierror",
        "AO" => "addon",
        "TT" => "ticket",
        "AX" => "restriction",
        "AF" => "blockedcall",
        "UW" => "widget",
        "TU" => "tutorial",
        "PP" => "matchend",
        "RM" => "marker",
        "SU" => "tracking",
        "RA" => "archaeology",
        "BC" => "carried",
        "RU" => "ruleset",
        "QB" => "questsdone",
        "EB" => "explored",
        "KT" => "titles",
        "IS" => "savedlock",
        "WX" => "weather",
        "RQ" => "recipeneeds",
        "SI" => "spellinfo",
        "SD" => "spelltext",
        "EL" => "bossloot",
        "AQ" => "auctionprice",
        "HD" => "hardcoredeath",
        "WH" => "who",
        "BO" => "scorerow",
        "IG" => "inspectgear",
        other => other,
    }
}

/// A PvP side index as a word: 0 Horde, 1 Alliance.
fn side(v: &str) -> &str {
    match v {
        "0" => "Horde",
        "1" => "Alliance",
        other => other,
    }
}

/// The client's QuestClassification as a word.
fn quest_class(v: &str) -> &str {
    match v {
        "0" => "important",
        "1" => "legendary",
        "2" => "campaign",
        "3" => "calling",
        "4" => "meta",
        "5" => "recurring",
        "6" => "questline",
        "7" => "normal",
        "8" => "bonus objective",
        "9" => "threat",
        "10" => "world quest",
        _ => "",
    }
}

/// The client's PlayerInteractionType as a word; other values stay numbers.
fn interaction(v: &str) -> String {
    let word = match v {
        "1" => "trade",
        "2" => "item",
        "3" => "gossip",
        "4" => "quest giver",
        "5" => "merchant",
        "6" => "flight master",
        "7" => "trainer",
        "8" => "banker",
        "10" => "guild banker",
        "11" => "registrar",
        "12" => "vendor",
        "17" => "mailbox",
        "18" => "spirit healer",
        "19" => "area spirit healer",
        "20" => "binder",
        "21" => "auctioneer",
        "22" => "stable master",
        "23" => "battlemaster",
        "24" => "transmogrifier",
        "37" => "player choice",
        "59" => "professions",
        "62" => "barber",
        _ => return format!("window {v}"),
    };
    word.to_string()
}

/// Number of ids in a comma list, 0 when empty.
fn list_len(v: &str) -> usize {
    v.split(',').filter(|s| !s.is_empty()).count()
}

fn carried_place(v: &str) -> &str {
    match v {
        "b" => "bags",
        "k" => "bank",
        "w" => "warband bank",
        "g" => "guild bank",
        "m" => "mailbox",
        "a" => "auctions",
        other => other,
    }
}

fn stack_totals(v: &str) -> (usize, i64) {
    v.split(',')
        .filter_map(|s| s.split_once(':'))
        .fold((0, 0), |(kinds, total), (_, n)| {
            (kinds + 1, total + n.parse::<i64>().unwrap_or(0))
        })
}

fn item_count(id: &str, min: &str, max: &str, names: &HashMap<String, String>) -> String {
    let item = label('i', id, names, "item");
    match (min.is_empty(), max.is_empty() || max == min) {
        (true, _) => item,
        (false, true) => format!("{item} x{min}"),
        (false, false) => format!("{item} x{min} to {max}"),
    }
}

fn power_costs(v: &str) -> String {
    v.split(',')
        .filter_map(|s| s.split_once(':'))
        .map(|(kind, cost)| format!("{cost} of power type {kind}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Copper as `1g 2s 3c`, empty when the field is empty.
fn coin(field: &str) -> String {
    if field.is_empty() {
        return String::new();
    }
    money(field).trim_start_matches('+').to_string()
}

/// `id:count,id:count` as `Name (id) xcount, ...`.
fn item_stacks(v: &str, names: &HashMap<String, String>) -> String {
    v.split(',')
        .filter(|s| !s.is_empty())
        .map(|s| match s.split_once(':') {
            Some((id, n)) => format!("{} x{n}", label('i', id, names, "item")),
            None => label('i', s, names, "item"),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// `spellID:rank,spellID` as `Name (id) rank N, ...`.
fn spell_ranks(v: &str, names: &HashMap<String, String>) -> String {
    v.split(',')
        .filter(|s| !s.is_empty())
        .map(|s| match s.split_once(':') {
            Some((id, rank)) => format!("{} rank {rank}", label('s', id, names, "spell")),
            None => label('s', s, names, "spell"),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// An action bar entry: a spell or item by name, anything else as `kind id`.
fn bar_entry(kind: &str, id: &str, names: &HashMap<String, String>) -> String {
    match kind {
        "" => "empty".to_string(),
        "spell" => label('s', id, names, "spell"),
        "item" => label('i', id, names, "item"),
        other => format!("{other} {id}"),
    }
}

/// `id,id` as `Name (id), ...`.
fn item_ids(v: &str, names: &HashMap<String, String>) -> String {
    v.split(',')
        .filter(|s| !s.is_empty())
        .map(|s| label('i', s, names, "item"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Non-empty parts joined by `, `.
fn parts(list: &[String]) -> String {
    list.iter()
        .filter(|s| !s.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(", ")
}

/// The client's LootRollType as a word.
fn roll_type(v: &str) -> &str {
    match v {
        "0" => "pass",
        "1" => "need",
        "2" => "greed",
        "3" => "disenchant",
        "4" => "transmog",
        _ => "",
    }
}

/// A dungeon or raid difficulty id as a word; ids not listed stay numbers.
pub fn difficulty_name(v: &str) -> String {
    let word = match v {
        "" => "unset",
        "1" => "normal",
        "2" => "heroic",
        "3" => "10 normal",
        "4" => "25 normal",
        "5" => "10 heroic",
        "6" => "25 heroic",
        "14" => "normal",
        "15" => "heroic",
        "16" => "mythic",
        _ => return v.to_string(),
    };
    word.to_string()
}

/// ` (suffix 12, bonus 1,2, upgrade 3)` for an item link's variant fields, empty when it has none.
fn variant(suffix: &str, bonus: &str, upgrade: &str) -> String {
    let mut out = Vec::new();
    if !suffix.is_empty() {
        out.push(format!("suffix {suffix}"));
    }
    if !bonus.is_empty() {
        out.push(format!("bonus {bonus}"));
    }
    if !upgrade.is_empty() {
        out.push(format!("upgrade {upgrade}"));
    }
    if out.is_empty() {
        String::new()
    } else {
        format!(" ({})", out.join(", "))
    }
}

/// The client's item bind type as a word.
fn bind_word(v: &str) -> &str {
    match v {
        "1" => "binds when picked up",
        "2" => "binds when equipped",
        "3" => "binds when used",
        "4" => "quest item",
        other => other,
    }
}

/// The client's item quality as a word.
fn quality_word(v: &str) -> &str {
    match v {
        "0" => "poor",
        "1" => "common",
        "2" => "uncommon",
        "3" => "rare",
        "4" => "epic",
        "5" => "legendary",
        "6" => "artifact",
        "7" => "heirloom",
        other => other,
    }
}

/// A loot method (`LM` field 1) as words.
fn loot_method(v: &str) -> &str {
    match v {
        "solo" => "solo",
        "group" => "group loot",
        "needbeforegreed" => "need before greed",
        "masterlooter" | "master" => "master looter",
        "freeforall" => "free for all",
        "roundrobin" => "round robin",
        "personal" => "personal loot",
        "" | "unknown" => "unknown loot rule",
        other => other,
    }
}

/// The client's reputation tier as a word.
fn reaction(v: &str) -> &str {
    match v {
        "1" => "hated",
        "2" => "hostile",
        "3" => "unfriendly",
        "4" => "neutral",
        "5" => "friendly",
        "6" => "honored",
        "7" => "revered",
        "8" => "exalted",
        other => other,
    }
}

/// What a spell does to the world, from its name (English clients only; the spell id table is
/// not available offline): a teleport (hearthstone, portal, summon stone), or gathering and
/// opening (herb, mine, skin, pickpocket, unlock, salvage).
fn spell_kind(name: &str) -> Option<&'static str> {
    let n = name.trim();
    if n.starts_with("Teleport:")
        || n.starts_with("Portal:")
        || n.starts_with("Teleport to")
        || n.starts_with("Portal to")
        || n.contains("Hearthstone")
        || n == "Astral Recall"
        || n == "Ritual of Summoning"
    {
        return Some("teleport");
    }
    match n {
        "Herb Gathering" | "Find Herbs" => Some("gathering: herb"),
        "Mining" | "Find Minerals" => Some("gathering: mine"),
        "Skinning" => Some("gathering: skin"),
        "Pick Pocket" => Some("gathering: pickpocket"),
        "Pick Lock" | "Opening" => Some("unlock"),
        "Disenchant" | "Prospecting" | "Milling" => Some("salvage"),
        _ => None,
    }
}

/// The client's unit reaction (1 to 8) toward you as a word.
fn reaction_to_creature(v: &str) -> &str {
    match v {
        "1" => "hostile (hated)",
        "2" => "hostile",
        "3" => "unfriendly",
        "4" => "neutral",
        "5" => "friendly",
        "6" => "friendly (honored)",
        "7" => "friendly (revered)",
        "8" => "friendly (exalted)",
        other => other,
    }
}

/// ` for 1g 2s` after a barber line, empty when the shop quoted no price.
fn barber_price(copper: &str) -> String {
    let price = coin(copper);
    if price.is_empty() {
        String::new()
    } else {
        format!(" for {price}")
    }
}

/// The client's BankType as a word.
fn bank_type(v: &str) -> &str {
    match v {
        "0" => "bank",
        "1" => "guild bank",
        "2" => "warband bank",
        _ => "bank",
    }
}

/// Signed copper as `+1g 2s 3c`.
fn money(field: &str) -> String {
    let Ok(c) = field.parse::<i64>() else {
        return field.to_string();
    };
    let sign = if c < 0 { '-' } else { '+' };
    let a = c.unsigned_abs();
    let (g, s, p) = (a / 10_000, a / 100 % 100, a % 100);
    let mut out = String::new();
    out.push(sign);
    if g > 0 {
        out.push_str(&format!("{g}g "));
    }
    if g > 0 || s > 0 {
        out.push_str(&format!("{s}s "));
    }
    out.push_str(&format!("{p}c"));
    out
}

/// A `TransmogCollectionType` number as the slot or weapon type it names.
fn transmog_category(v: &str) -> String {
    const NAMES: [&str; 30] = [
        "none",
        "Head",
        "Shoulder",
        "Back",
        "Chest",
        "Shirt",
        "Tabard",
        "Wrist",
        "Hands",
        "Waist",
        "Legs",
        "Feet",
        "Wand",
        "One-hand axe",
        "One-hand sword",
        "One-hand mace",
        "Dagger",
        "Fist weapon",
        "Shield",
        "Held in off hand",
        "Two-hand axe",
        "Two-hand sword",
        "Two-hand mace",
        "Staff",
        "Polearm",
        "Bow",
        "Gun",
        "Crossbow",
        "Warglaives",
        "Paired weapons",
    ];
    match v.parse::<usize>().ok().and_then(|i| NAMES.get(i)) {
        Some(n) => (*n).to_string(),
        None => format!("category {v}"),
    }
}

/// A `TransmogOutfitSlot` number as its name.
fn outfit_slot(v: &str) -> String {
    const NAMES: [&str; 15] = [
        "head",
        "right shoulder",
        "left shoulder",
        "back",
        "chest",
        "tabard",
        "shirt",
        "wrist",
        "hands",
        "waist",
        "legs",
        "feet",
        "main hand",
        "off hand",
        "ranged",
    ];
    match v.parse::<usize>().ok().and_then(|i| NAMES.get(i)) {
        Some(n) => (*n).to_string(),
        None => format!("slot {v}"),
    }
}

/// A battle pet type number as its family.
fn pet_type(v: &str) -> &str {
    match v {
        "1" => "humanoid",
        "2" => "dragonkin",
        "3" => "flying",
        "4" => "undead",
        "5" => "critter",
        "6" => "magic",
        "7" => "elemental",
        "8" => "beast",
        "9" => "aquatic",
        "10" => "mechanical",
        other => other,
    }
}

/// `speciesID:level:quality,...` of a pet battle team as `species 12 L5 q3, ...`.
fn pet_team(v: &str) -> String {
    v.split(',')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut it = p.splitn(3, ':');
            let (species, level, quality) = (
                it.next().unwrap_or(""),
                it.next().unwrap_or(""),
                it.next().unwrap_or(""),
            );
            let mut s = format!("species {species}");
            if !level.is_empty() {
                s.push_str(&format!(" L{level}"));
            }
            if !quality.is_empty() {
                s.push_str(&format!(" q{quality}"));
            }
            s
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Group roles as words from the addon's letters: t tank, h healer, d damage.
fn role_words(v: &str) -> String {
    v.chars()
        .filter_map(|c| match c {
            't' => Some("tank"),
            'h' => Some("healer"),
            'd' => Some("damage"),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// `1` as yes, `0` as no, anything else (empty, unknown) as a question mark.
fn yes_no(v: &str) -> &str {
    match v {
        "1" => "yes",
        "0" => "no",
        _ => "?",
    }
}

/// A community type number as a word.
fn club_type(v: &str) -> &str {
    match v {
        "0" => "Battle.net community",
        "1" => "community",
        "2" => "guild",
        "3" => "other community",
        "" => "community",
        other => other,
    }
}

/// A community role number as a word.
fn club_role(v: &str) -> &str {
    match v {
        "1" => "owner",
        "2" => "leader",
        "3" => "moderator",
        "4" => "member",
        other => other,
    }
}

/// Key for the names map: `n` npc, `s` spell, `i` item.
pub fn name_key(kind: char, id: &str) -> String {
    format!("{kind}{id}")
}

/// The name-defining codes and the kind of name each one sets.
pub fn name_kind(code: &str) -> Option<char> {
    match code {
        "N" => Some('n'),
        "SN" => Some('s'),
        "IN" => Some('i'),
        "CN" => Some('c'),
        "QN" => Some('q'),
        "KN" => Some('k'),
        "WN" => Some('w'),
        "AN" => Some('a'),
        "GN" => Some('o'),
        "FN" => Some('f'),
        "BN" => Some('p'),
        "TN" => Some('t'),
        "TY" => Some('g'),
        _ => None,
    }
}

/// The object a fishing cast loots (objectID 35591 on WoW Forever).
const FISHING_BOBBER: &str = "35591";

/// The three Legacy trees by their trait tree ID.
fn legacy_tree(id: &str) -> String {
    match id {
        "1187" => "Professions".into(),
        "1188" => "Adventure".into(),
        "1189" => "Progression".into(),
        other => format!("tree {other}"),
    }
}

/// The client's ReportType as a word; other values stay numbers.
fn report_type(v: &str) -> String {
    let word = match v {
        "0" => "chat",
        "1" => "in world",
        "2" => "community finder posting",
        "3" => "community finder applicant",
        "4" => "group finder posting",
        "5" => "group finder applicant",
        "6" => "community member",
        "7" => "group member",
        "8" => "friend",
        "9" => "pet",
        "10" => "battle pet",
        "11" => "calendar",
        "12" => "mail",
        "13" => "PvP",
        "14" => "PvP scoreboard",
        "15" => "PvP group member",
        "16" => "crafting order",
        "17" => "recent ally",
        _ => return format!("type {v}"),
    };
    word.to_string()
}

/// The client's SendReportResult as a word.
fn report_result(v: &str) -> String {
    match v {
        "0" => "sent".into(),
        "1" => "error".into(),
        "2" => "too many reports".into(),
        "3" => "needs a chat line".into(),
        "4" => "needs a chat line or voice".into(),
        "5" => "needs a screenshot".into(),
        other => format!("result {other}"),
    }
}

/// A raid target marker index as its icon.
fn raid_icon(v: &str) -> String {
    match v {
        "1" => "star".into(),
        "2" => "circle".into(),
        "3" => "diamond".into(),
        "4" => "triangle".into(),
        "5" => "moon".into(),
        "6" => "square".into(),
        "7" => "cross".into(),
        "8" => "skull".into(),
        other => format!("marker {other}"),
    }
}

fn label(kind: char, id: &str, names: &HashMap<String, String>, fallback: &str) -> String {
    match names.get(&name_key(kind, id)) {
        Some(n) => format!("{n} ({id})"),
        None => format!("{fallback} {id}"),
    }
}

const SAME_DEATH_MS: i64 = 3000;

fn same_death(last: &mut Option<i64>, ms: i64) -> bool {
    let again = last.is_some_and(|l| (0..=SAME_DEATH_MS).contains(&(ms - l)));
    if !again {
        *last = Some(ms);
    }
    again
}

/// Reads one character's death lines in time order and tells a death written twice from two deaths. Lines before
/// 2026-11-22 could hold one death as `D`, `DL`, `D`, `DL` and then its recap twice.
#[derive(Default)]
pub struct Deaths {
    died: Option<i64>,
    blow: Option<i64>,
    repeat: bool,
}

impl Deaths {
    /// Reads a `D` line; false when it is a second event for the death before it.
    pub fn died(&mut self, ms: i64) -> bool {
        self.repeat = same_death(&mut self.died, ms);
        !self.repeat
    }

    /// True when the last `D` read was a second event for the same death, so the `DL` after it marks the same place.
    pub fn repeated(&self) -> bool {
        self.repeat
    }

    /// Reads a killing blow (`DR` pos 1); false when it is the same recap written again.
    pub fn killing_blow(&mut self, ms: i64) -> bool {
        !same_death(&mut self.blow, ms)
    }
}

/// The name of what killed a character when the killing blow came from the world: the cause in `DR` field 17, or
/// "The environment" when the line names none.
pub fn environmental_killer(f: &[String]) -> Option<&'static str> {
    let g = |i: usize| f.get(i).map(String::as_str).unwrap_or("");
    if g(3) != "1" {
        return None;
    }
    match g(17) {
        "Falling" => Some("Fall"),
        "Drowning" => Some("Drowning"),
        "Fatigue" => Some("Fatigue"),
        "Fire" => Some("Fire"),
        "Lava" => Some("Lava"),
        "Slime" => Some("Slime"),
        _ => Some("The environment"),
    }
}

/// A spot on a map from the `P` units: the map, then the map percent to one decimal.
fn spot(map: &str, x: &str, y: &str) -> String {
    if map.is_empty() {
        return "an unknown place".into();
    }
    match (x.parse::<f64>(), y.parse::<f64>()) {
        (Ok(x), Ok(y)) => format!("map {map} at {:.1},{:.1}", x / 100.0, y / 100.0),
        _ => format!("map {map}"),
    }
}

/// The screenshot moment codes of an `SS` line.
fn shot_moment(v: &str) -> &str {
    match v {
        "lv" => "level up",
        "de" => "death",
        "bk" => "boss kill",
        "ep" => "epic item",
        "nc" => "new collectible",
        "dw" => "duel won",
        "pk" => "PvP kill",
        other => other,
    }
}

fn loot_items(v: &str, names: &HashMap<String, String>) -> String {
    v.split(',')
        .filter(|t| !t.is_empty())
        .map(|t| {
            let (what, qty) = t.split_once('x').unwrap_or((t, ""));
            let name = match what.strip_prefix('c') {
                _ if what == "m" => "coin".to_string(),
                Some(cur) => format!("currency {cur}"),
                None => label('i', what, names, "item"),
            };
            if qty.is_empty() || what == "m" {
                name
            } else {
                format!("{name} x{qty}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn details(code: &str, f: &[String], names: &HashMap<String, String>) -> String {
    let g = |i: usize| f.get(i).map(String::as_str).unwrap_or("");
    let spell = |i: usize| label('s', g(i), names, "spell");
    let item = |i: usize| label('i', g(i), names, "item");
    match code {
        "S" => {
            let mut s = f.iter().take(8).cloned().collect::<Vec<_>>().join(" ");
            if !g(8).is_empty() {
                s.push_str(&format!(" region {}", g(8)));
            }
            if !g(9).is_empty() {
                s.push_str(&format!(" toc {}", g(9)));
            }
            if !g(10).is_empty() && !g(11).is_empty() {
                s.push_str(&format!(" xp {}/{}", g(10), g(11)));
            }
            s
        }
        "ES" | "D" | "R" | "U" => String::new(),
        "Z" => {
            let zone = g(4);
            let sub = g(5);
            let mut s = if sub.is_empty() {
                zone.to_string()
            } else {
                format!("{zone} - {sub}")
            };
            let it = g(2);
            if !it.is_empty() && it != "none" {
                s.push_str(&format!(" ({it})"));
            }
            s
        }
        "W" => {
            let mode = match g(0) {
                "f" => "running",
                "w" => "walking",
                "m" => "mounted",
                "s" => "swimming",
                "a" => "flying",
                other => other,
            };
            let jumps = if g(3) == "1" { "jump" } else { "jumps" };
            let mut s = format!("{mode} {} yd in {}s, {} {jumps}", g(1), g(2), g(3));
            if !g(4).is_empty() {
                s.push_str(&format!(" (at {}s)", g(4).replace(',', "s, ")));
            }
            s
        }
        "T" => {
            if g(0) == "1" {
                "took off".into()
            } else {
                format!("landed after {}s", g(4))
            }
        }
        "C" => {
            let mut s = spell(0);
            if let Some(kind) = spell_kind(
                names
                    .get(&name_key('s', g(0)))
                    .map(String::as_str)
                    .unwrap_or(""),
            ) {
                s.push_str(&format!(" [{kind}]"));
            }
            if !g(1).is_empty() {
                s.push_str(&format!(" ({}s cast)", g(1)));
            }
            s
        }
        "CH" => format!("{} for {}s", spell(0), g(1)),
        "CI" => format!("{} after {}s", spell(0), g(1)),
        "AFK" => if g(0) == "1" { "away" } else { "back" }.into(),
        "RX" => {
            if g(0) == "1" {
                format!("{} rested xp", g(1))
            } else {
                "no rested xp".into()
            }
        }
        "RO" => format!(
            "{} rested xp of {}{}",
            g(0),
            g(1),
            if g(2) == "1" { ", resting" } else { "" }
        ),
        "GP" => {
            if g(0) == "1" {
                format!("joined a group of {}", g(1))
            } else {
                "solo".into()
            }
        }
        "GM" => {
            if g(0) == "1" {
                format!("{} joined", g(1))
            } else {
                format!("{} left after {}s together", g(1), g(2))
            }
        }
        "WD" => format!("\"{}\" x{}", g(0), g(1)),
        "EM" => g(0).to_lowercase(),
        "DM" | "DS" | "DP" => {
            let what = match g(0) {
                "d" => "damage",
                "h" => "healing",
                "t" => "damage taken",
                "a" => "absorbs",
                "i" => "interrupts",
                "x" => "dispels",
                other => other,
            };
            match code {
                "DM" => format!("{what} {} ({}/s over {}s)", g(1), g(2), g(3)),
                "DS" => format!("{what} {} from {}", g(2), spell(1)),
                _ => format!("{what} {} by {} ({})", g(3), g(1), g(2).to_lowercase()),
            }
        }
        "DL" => format!("map {} at {}, {}", g(0), g(1), g(2)),
        "RZ" => format!("from {}", g(0)),
        "SH" => "took spirit healer resurrection".into(),
        "DU" => format!("{} asked you to duel", g(0)),
        "DW" => {
            if g(0) == "1" {
                format!("won against {}", g(1))
            } else {
                format!("lost to {}", g(1))
            }
        }
        "EX" => format!("{} ({} xp)", g(0), g(1)),
        "XR" => format!("+{} rested bonus", g(0)),
        "MT" => {
            let name = g(1).to_lowercase();
            if g(0) == "1" {
                format!("{name} started")
            } else {
                format!("{name} ended after {}s", g(2))
            }
        }
        "DB" => format!("average {}%, lowest {}%, {} broken", g(0), g(1), g(2)),
        "RE" => money(&format!("-{}", g(0))),
        "NI" => {
            let kind = match g(0) {
                "g" => "talked to",
                "v" => "vendor",
                "f" => "flight master",
                "t" => "trainer",
                "b" => "banker",
                "a" => "auctioneer",
                "q" => "quest from",
                other => other,
            };
            let mut s = format!("{kind} {} ({})", g(2), g(1));
            if !g(3).is_empty() {
                s.push_str(&format!(", {}", g(3)));
            }
            match g(4) {
                "1" => s.push_str(", repairs"),
                "0" => s.push_str(", no repairs"),
                _ => {}
            }
            s
        }
        "VS" => {
            let list: Vec<String> = g(2)
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|row| {
                    let mut it = row.splitn(5, ':');
                    let (id, price, stack, limit, cost) = (
                        it.next().unwrap_or(""),
                        it.next().unwrap_or(""),
                        it.next().unwrap_or(""),
                        it.next().unwrap_or(""),
                        it.next().unwrap_or(""),
                    );
                    let mut s = label('i', id, names, "item");
                    if stack != "1" && !stack.is_empty() {
                        s.push_str(&format!(" x{stack}"));
                    }
                    if price != "0" && !price.is_empty() {
                        s.push_str(&format!(" {}", coin(price)));
                    }
                    if !cost.is_empty() {
                        s.push_str(&format!(" +{cost}"));
                    }
                    if limit != "-1" && !limit.is_empty() {
                        s.push_str(&format!(" ({limit} left)"));
                    }
                    s
                })
                .collect();
            format!(
                "{} sells {} items: {}",
                label('n', g(0), names, "npc"),
                g(1),
                list.join(", ")
            )
        }
        "QN" => format!("{} ({})", g(1), g(0)),
        "LS" | "LR" => spell(0),
        "TL" => format!("{} talent point change", g(0)),
        "HB" => {
            if g(1) == "s" {
                format!("bound to {} (found at login)", g(0))
            } else {
                format!("bound to {}", g(0))
            }
        }
        "HK" => format!("{} total (+{})", g(0), g(1)),
        "HN" => {
            let mut s = format!("+{}", g(0));
            if !g(1).is_empty() {
                s.push_str(&format!(" for {}", g(1)));
                if !g(2).is_empty() {
                    s.push_str(&format!(" (rank {})", g(2)));
                }
            }
            s
        }
        "PK" => {
            let mut bits = Vec::new();
            if !g(3).is_empty() {
                bits.push(format!("level {}", g(3)));
            }
            if !g(2).is_empty() {
                bits.push(g(2).to_lowercase());
            }
            if !g(1).is_empty() {
                bits.push(g(1).to_lowercase());
            }
            if !g(5).is_empty() {
                bits.push(g(5).to_string());
            }
            let by = match g(4) {
                "1" => "you",
                "0" => "party",
                _ => "unknown",
            };
            format!("{} ({}) by {by}", g(0), bits.join(" "))
        }
        "AV" => {
            let kind = match g(0) {
                "seller" => "sold",
                "buyer" => "bought",
                "seller_temp_invoice" => "sale pending",
                other => other,
            };
            let mut s = format!("{kind} {}", g(1));
            if !g(2).is_empty() && g(2) != "1" {
                s.push_str(&format!(" x{}", g(2)));
            }
            if !g(3).is_empty() {
                s.push_str(&format!(" for {}", coin(g(3))));
            }
            if !g(5).is_empty() && g(5) != "0" {
                s.push_str(&format!(", deposit {}", coin(g(5))));
            }
            if !g(6).is_empty() && g(6) != "0" {
                s.push_str(&format!(", house cut {}", coin(g(6))));
            }
            match g(7) {
                "i" => s.push_str(" (seen in the inbox)"),
                "o" => s.push_str(" (mail opened)"),
                "t" => s.push_str(" (taken)"),
                _ => {}
            }
            s
        }
        "PV" => if g(0) == "1" { "flagged" } else { "unflagged" }.into(),
        "PT" => {
            if g(0).is_empty() {
                "dismissed".into()
            } else {
                format!("{} ({})", g(0), g(1))
            }
        }
        "SS" => {
            if g(0).is_empty() {
                String::new()
            } else if !g(2).is_empty() {
                format!("{} (taken by Scarlet: {})", g(0), shot_moment(g(2)))
            } else if g(1) == "c" {
                format!("{} (a Screenshot call was seen)", g(0))
            } else {
                g(0).to_string()
            }
        }
        "TP" => format!("{}s total, {}s this level", g(0), g(1)),
        "RL" => {
            let how = match g(0) {
                "0" => "passed",
                "1" => "need",
                "2" => "greed",
                other => other,
            };
            format!("{how} on {}", item(1))
        }
        "ST" => format!(
            "level {} gold {} str {} agi {} sta {} int {} spi {} armor {} hp {} power {} ilvl {}",
            g(0),
            coin(g(1)),
            g(2),
            g(3),
            g(4),
            g(5),
            g(6),
            g(7),
            g(8),
            g(9),
            g(10)
        ),
        "GR" => format!("preset {} hardcore {}", g(0), g(1)),
        "DR" => {
            let world = match g(17) {
                "Falling" => Some("a fall"),
                "Drowning" => Some("drowning"),
                "Fatigue" => Some("fatigue"),
                "Fire" => Some("fire"),
                "Lava" => Some("lava"),
                "Slime" => Some("slime"),
                _ => None,
            }
            .filter(|_| g(3) == "1");
            let named_spell = !g(0).is_empty() && g(0) != "?";
            let what = match (g(16), world) {
                ("SWING_DAMAGE", _) => "a melee swing".to_string(),
                ("RANGE_DAMAGE", _) if named_spell => format!("a ranged shot ({})", spell(0)),
                ("RANGE_DAMAGE", _) => "a ranged shot".to_string(),
                (_, Some(w)) => w.to_string(),
                _ if named_spell => spell(0),
                _ if g(0) == "?" => "a hit the client did not name".to_string(),
                _ => "hit".to_string(),
            };
            let source = if g(2).is_empty() || g(2) == "?" {
                "unknown".to_string()
            } else if g(10) == "c" && !g(22).is_empty() && g(22) != "?" {
                match names.get(&name_key('n', g(22))) {
                    Some(n) => format!("{n} ({})", g(22)),
                    None => format!("{} (npc {})", g(2), g(22)),
                }
            } else {
                g(2).to_string()
            };
            let mut s = format!("{what} for {}", g(1));
            if world.is_none() {
                if g(3) == "1" {
                    s.push_str(" from the environment");
                } else {
                    s.push_str(&format!(" from {source}"));
                }
            }
            if g(4) == "1" {
                s.push_str(", the killing blow");
            }
            if !g(5).is_empty() && g(5) != "0" && g(5) != "-1" {
                s.push_str(&format!(" ({} overkill)", g(5)));
            }
            for (i, word) in [(18, "absorbed"), (19, "resisted"), (20, "blocked")] {
                if !g(i).is_empty() && g(i) != "0" && g(i) != "?" {
                    s.push_str(&format!(", {} {word}", g(i)));
                }
            }
            for (letter, word) in [('a', "avoidable"), ('d', "deadly"), ('h', "caster hidden")] {
                if g(21).contains(letter) {
                    s.push_str(&format!(", {word}"));
                }
            }
            if !g(8).is_empty() && g(8) != "0" {
                s.push_str(&format!(", {}s before death", g(8)));
            }
            // The killing blow's source, when it was in sight, and the recap's length.
            let kind = match g(10) {
                "p" => "a player",
                "c" => "a creature",
                _ => "",
            };
            if !kind.is_empty() {
                let mut bits = Vec::new();
                if g(11) == "-1" {
                    bits.push("level ??".to_string());
                } else if !g(11).is_empty() {
                    bits.push(format!("level {}", g(11)));
                }
                if !g(12).is_empty() && g(12) != "normal" {
                    bits.push(g(12).to_lowercase());
                }
                if !g(13).is_empty() {
                    bits.push(g(13).to_lowercase());
                }
                if !g(14).is_empty() {
                    bits.push(format!("<{}>", g(14)));
                }
                s.push_str(&format!(", the killer was {kind}"));
                if !bits.is_empty() {
                    s.push_str(&format!(" ({})", bits.join(", ")));
                }
            }
            if !g(15).is_empty() {
                s.push_str(&format!(", {} hits in the recap", g(15)));
            }
            s
        }
        "TE" => {
            let by = match g(0) {
                "m" => "a summon".to_string(),
                "i" => match (g(2).is_empty(), g(1).is_empty()) {
                    (false, false) => format!("{} (spell {})", item(2), g(1)),
                    (false, true) => item(2),
                    _ => spell(1),
                },
                "o" => format!("{} (a spell you do not know, such as a portal)", spell(1)),
                _ => spell(1),
            };
            let mut s = format!(
                "teleported by {by} from {} to {}",
                spot(g(3), g(4), g(5)),
                spot(g(6), g(7), g(8))
            );
            if !g(9).is_empty() {
                s.push_str(&format!(", {} yards", g(9)));
            }
            if g(11) == "d" {
                s.push_str(", on the same map");
            }
            s
        }
        "NP" => {
            let who = if g(0) == "o" {
                format!("object {}", g(1))
            } else {
                label('n', g(1), names, "npc")
            };
            let how = match g(5) {
                "s" => "seen",
                "t" => "talked to",
                "l" => "looted",
                other => other,
            };
            format!("{who} {how} from map {} cell {},{}", g(2), g(3), g(4))
        }
        "CU" => {
            let n = g(1);
            let n = if n.starts_with('-') {
                n.to_string()
            } else {
                format!("+{n}")
            };
            format!("{n} {} (now {})", label('c', g(0), names, "currency"), g(2))
        }
        "CR" => {
            let crit = if g(2) == "1" { ", crit" } else { "" };
            let mut s = format!("{} x{}{crit}", item(0), g(1));
            // Fields past multicraft only exist on newer lines; old 4-field
            // lines render exactly as before.
            if f.len() > 4 {
                let multi: i64 = g(3).parse().unwrap_or(0);
                if multi > 1 {
                    s.push_str(&format!(", {multi}x multicraft"));
                }
                if !g(4).is_empty() {
                    s.push_str(&format!(" (recipe {})", spell(4)));
                }
                if !g(5).is_empty() && g(5) != "0" {
                    s.push_str(&format!(", quality {}", g(5)));
                }
                if g(6) == "1" {
                    s.push_str(", bonus");
                }
                let conc: i64 = g(7).parse().unwrap_or(0);
                if conc > 0 {
                    s.push_str(&format!(", {conc} concentration spent"));
                }
                if g(8) == "1" {
                    s.push_str(", ingenuity");
                }
                if g(9) == "1" {
                    s.push_str(", first craft");
                }
                let returned: i64 = g(10).parse().unwrap_or(0);
                if returned > 0 {
                    s.push_str(&format!(", {returned} reagent stacks returned"));
                }
                if g(11) == "1" {
                    s.push_str(", enchant");
                }
            }
            s
        }
        "CC" if g(0) == "?" => {
            if g(3).is_empty() {
                "lost control (the client hid what)".to_string()
            } else {
                format!("lost control (the client hid what; priority {})", g(3))
            }
        }
        "CC" => format!("{} by {} for {}s", g(0).to_lowercase(), spell(1), g(2)),
        "NC" => {
            let what = if g(2).is_empty() {
                g(1).to_string()
            } else {
                g(2).to_string()
            };
            format!("{} {what}", g(0)).trim_end().to_string()
        }
        "TS" => {
            if g(0) == "1" {
                let mut s = if g(2).is_empty() {
                    format!("profession {}", g(1))
                } else {
                    format!("{} ({})", g(2), g(1))
                };
                if !g(3).is_empty() {
                    s.push_str(&format!(", skill {}/{}", g(3), g(4)));
                }
                s
            } else {
                "closed".into()
            }
        }
        "CB" => format!("recipe {}", spell(0)),
        "CS" => {
            if g(0) == "1" {
                "stopped (scrapping)".into()
            } else {
                "stopped".into()
            }
        }
        "CW" => {
            let cur = label('c', g(0), names, "currency");
            let first = if g(2) == "1" { " (first craft)" } else { "" };
            if g(3).is_empty() {
                format!("{cur} x{}{first}", g(1))
            } else {
                format!("{cur} x{}{first} for {}", g(1), spell(3))
            }
        }
        "SC" => {
            let why = match g(0) {
                "s" => "session baseline",
                "g" => "gear",
                "l" => "level",
                "t" => "talents",
                "a" => "auras",
                "o" => "other",
                other => other,
            };
            format!("{} {} ({})", g(1), g(2), why)
        }
        "SL" => {
            let mut s = format!("{} rank {}", label('k', g(0), names, "skill"), g(1));
            if !g(2).is_empty() {
                s.push_str(&format!("/{}", g(2)));
            }
            if !g(3).is_empty() {
                s.push_str(&format!(" +{} modifier", g(3)));
            }
            if g(4) == "s" {
                s.push_str(" (baseline)");
            }
            s
        }
        "WS" => {
            let v = g(1);
            let v = if v.contains("|T") {
                stat_number(v)
                    .map(|c| money(&c.to_string()))
                    .unwrap_or_else(|| plain_value(v))
            } else {
                v.to_string()
            };
            format!("{} = {}", label('w', g(0), names, "statistic"), v)
        }
        "WN" | "KN" | "FN" | "BN" => format!("{} ({})", g(1), g(0)),
        "AP" => format!("{} points, {}/{} challenges", g(0), g(1), g(2)),
        "AN" => format!("{} ({} points, {})", g(1), g(2), g(0)),
        "AE" => format!("{} ({})", g(1), label('a', g(0), names, "achievement")),
        "LV" => g(0).to_string(),
        "LB" => {
            let mut parts = vec![format!("{} earned", g(0))];
            for (i, what) in [
                (1, "available"),
                (2, "spent"),
                (3, "seasonal cap"),
                (4, "max available"),
            ] {
                if !g(i).is_empty() {
                    parts.push(format!("{} {what}", g(i)));
                }
            }
            let trees: Vec<String> = [(5, "1187"), (6, "1188"), (7, "1189")]
                .into_iter()
                .filter(|(i, _)| !g(*i).is_empty())
                .map(|(i, tree)| format!("{} {}", legacy_tree(tree), g(i)))
                .collect();
            let mut s = parts.join(", ");
            if !trees.is_empty() {
                s.push_str(&format!(" (spent in {})", trees.join(", ")));
            }
            if g(8) == "s" {
                s.push_str(" (baseline)");
            }
            s
        }
        "LA" => format!(
            "{} paid {} Legacy points",
            label('a', g(0), names, "achievement"),
            g(1)
        ),
        "LN" => {
            let what = if !g(7).is_empty() {
                spell(7)
            } else if !g(9).is_empty() {
                g(9).to_string()
            } else {
                format!("node {}", g(1))
            };
            let max = if g(6).is_empty() {
                String::new()
            } else {
                format!(" of {}", g(6))
            };
            if g(8) == "s" {
                format!(
                    "{}: {what}: rank {}{max} (baseline)",
                    legacy_tree(g(0)),
                    g(3)
                )
            } else {
                format!(
                    "{}: {what}: rank {} to {}{max}",
                    legacy_tree(g(0)),
                    g(4),
                    g(3)
                )
            }
        }
        "LO" => {
            let slots = g(2);
            let from = match g(0) {
                "c" => format!("from {} ({slots} slots)", label('n', g(1), names, "npc")),
                "g" if g(1) == FISHING_BOBBER => format!("from the fishing bobber ({slots} slots)"),
                "g" => format!("from object {} ({slots} slots)", g(1)),
                "i" if !g(1).is_empty() => format!("from opening {} ({slots} slots)", item(1)),
                "i" => format!("from a bag item ({slots} slots)"),
                _ => format!("from elsewhere ({slots} slots)"),
            };
            let items = loot_items(g(3), names);
            if items.is_empty() {
                from
            } else {
                format!("{from}: {items}")
            }
        }
        "LW" => {
            let slots = g(3);
            let from = match g(1) {
                "c" => format!("from {} ({slots} slots)", label('n', g(2), names, "npc")),
                "g" if g(2) == FISHING_BOBBER => format!("from the fishing bobber ({slots} slots)"),
                "g" => format!("from object {} ({slots} slots)", g(2)),
                _ => format!("from elsewhere ({slots} slots)"),
            };
            let items = loot_items(g(4), names);
            let mut s = if items.is_empty() {
                format!("{}'s Scarlet read {from}", g(0))
            } else {
                format!("{}'s Scarlet read {from}: {items}", g(0))
            };
            if g(5) == "1" {
                s.push_str(" (already read)");
            }
            if let Ok(late) = g(6).parse::<i64>()
                && late > 0
            {
                s.push_str(&format!(", sent {late}s late"));
            }
            s
        }
        "PR" => {
            if let Some(event) = g(0).strip_prefix("?e:") {
                format!("content probe: event {event} fired")
            } else if let Some(structure) = g(0).strip_prefix("?k:") {
                if g(2).is_empty() {
                    format!("content probe: fields of {structure}")
                } else {
                    format!("content probe: fields of {structure}: {}", g(2))
                }
            } else if let Some(function) = g(0).strip_prefix("?f:") {
                match (g(1), g(2).is_empty()) {
                    ("1", true) => format!("content probe: function {function} returned something"),
                    ("1", false) => format!(
                        "content probe: function {function} returned something ({})",
                        g(2)
                    ),
                    (_, false) if g(2) == "missing" => {
                        format!("content probe: this client has no function {function}")
                    }
                    (_, false) if g(2) == "error" => {
                        format!("content probe: function {function} failed when called")
                    }
                    (_, false) if g(2) == "noarg" => format!(
                        "content probe: function {function} was not called, there was nothing to pass it"
                    ),
                    _ => format!("content probe: function {function} returned nothing"),
                }
            } else if g(1) == "1" {
                format!("{} available", g(0))
            } else {
                format!("{} missing", g(0))
            }
        }
        "TW" => {
            if g(1) == "0" {
                format!("slot {} emptied", g(0))
            } else {
                let dur = g(2)
                    .parse::<f64>()
                    .map(|d| format!(" for {d}s"))
                    .unwrap_or_default();
                format!("slot {}: {}{dur}", g(0), spell(1))
            }
        }
        "IL" => {
            if g(0) == "1" {
                let name = if g(2).is_empty() { "instance" } else { g(2) };
                format!("saved to {name} ({}/{} bosses), {}s left", g(3), g(4), g(1))
            } else {
                "left the saved instance".into()
            }
        }
        "SKN" => {
            if g(0) == "1" {
                "your corpse was skinned (free repop available)".into()
            } else {
                "your corpse was skinned".into()
            }
        }
        "SR" => format!(
            "can self-res: {}",
            if g(0) == "1" { item(1) } else { spell(1) }
        ),
        "RD" => {
            let level = match g(1) {
                "0" => "optimal",
                "1" => "medium",
                "2" => "easy",
                "3" => "trivial",
                _ => "unrated",
            };
            let mut s = format!("{} is {level}", spell(0));
            let ups = g(2);
            if !ups.is_empty() && ups != "0" {
                s.push_str(&format!(", {ups} skill-ups"));
            }
            if g(3) == "0" {
                s.push_str(", no skill-up");
            }
            for (i, what) in [
                (4, "recraft"),
                (5, "salvage"),
                (6, "enchant"),
                (7, "first craft"),
            ] {
                if g(i) == "1" {
                    s.push_str(&format!(", {what}"));
                }
            }
            s
        }
        "RT" => {
            let what = if g(1) == "1" {
                "tracking"
            } else {
                "stopped tracking"
            };
            let mut s = format!("{what} {}", spell(0));
            if g(3) == "1" {
                s.push_str(" (recraft)");
            }
            if g(2) == "s" {
                s.push_str(" (baseline)");
            }
            s
        }
        "PG" => {
            let item_part = if g(1) == "0" {
                "emptied".to_string()
            } else {
                item(1)
            };
            let mut s = format!("slot {}: {item_part}, profession {}", g(0), g(2));
            if !g(3).is_empty() {
                s.push_str(&format!(", {}", label('k', g(3), names, "skill line")));
            }
            s
        }
        "PS" => format!(
            "{}: {} specialization points to spend",
            label('k', g(0), names, "skill line"),
            g(1)
        ),
        "PU" => format!(
            "specializations unlocked for {}",
            label('k', g(0), names, "skill line")
        ),
        "PO" => {
            let who = if g(2).is_empty() {
                format!("profession {}", g(0))
            } else {
                g(2).to_string()
            };
            if g(1) == "0" {
                format!("no personal orders for {who}")
            } else {
                format!("{} personal orders for {who}", g(1))
            }
        }
        "CO" => {
            let kind = if g(0).is_empty() {
                "fulfilled order".to_string()
            } else {
                format!("fulfilled {} order", g(0))
            };
            let tip = money(g(3));
            format!(
                "{kind} for {} x{}, tip {}",
                g(1),
                g(2),
                tip.trim_start_matches('+')
            )
        }
        "QI" => {
            let mut s = format!("{}: level {}", label('q', g(0), names, "quest"), g(1));
            match g(2) {
                "1" => s.push_str(", daily"),
                "2" => s.push_str(", weekly"),
                _ => {}
            }
            let class = quest_class(g(3));
            if !class.is_empty() {
                s.push_str(&format!(", {class}"));
            }
            if !g(5).is_empty() && g(5) != "0" {
                s.push_str(&format!(", group of {}", g(5)));
            }
            if !g(6).is_empty() {
                s.push_str(&format!(", campaign {}", g(6)));
            }
            if !g(7).is_empty() {
                s.push_str(&format!(", timed {}s", g(7)));
            }
            if g(8) == "s" {
                s.push_str(" (in the log at login)");
            }
            s
        }
        "QP" => {
            let quest = label('q', g(0), names, "quest");
            let what = if g(4) == "criteria" {
                format!("criterion {}", g(1))
            } else {
                format!("objective {}", g(1))
            };
            let text = if g(5).is_empty() {
                String::new()
            } else {
                format!(" ({})", g(5))
            };
            format!("{quest} {what}{text}: {}/{}", g(2), g(3))
        }
        "QS" => {
            let state = match g(1) {
                "c" => "complete",
                "u" => "no longer complete",
                "f" => "failed",
                "a" => "can be turned in anywhere",
                other => other,
            };
            format!("{} {state}", label('q', g(0), names, "quest"))
        }
        "QA" => format!("{} shared {}", g(1), label('q', g(0), names, "quest")),
        "QD" => {
            let stage = match g(0) {
                "g" => "greeting from",
                "p" => "progress with",
                "c" => "reward window of",
                other => other,
            };
            let mut s = format!("{stage} {}", label('n', g(2), names, "npc"));
            if !g(1).is_empty() {
                s.push_str(&format!(" for {}", label('q', g(1), names, "quest")));
            }
            s
        }
        "QR" => format!(
            "took {} for {} (choice {} of {})",
            item(2),
            label('q', g(0), names, "quest"),
            g(1),
            g(3)
        ),
        "QW" => format!(
            "{} {}",
            if g(1) == "1" {
                "tracking"
            } else {
                "stopped tracking"
            },
            label('q', g(0), names, "quest")
        ),
        "QY" => match g(0) {
            "c" => "created".to_string(),
            "d" => "ended".to_string(),
            "j" => "joined".to_string(),
            "l" => "left".to_string(),
            other => other.to_string(),
        },
        "QK" => {
            let state = match g(2) {
                "1" => "complete",
                "2" => "in progress",
                "3" => "stalled",
                _ => "unknown",
            };
            let mut s = format!("{} ({}): {state}", g(1), g(0));
            if !g(3).is_empty() {
                s.push_str(&format!(", chapter {}", g(3)));
            }
            s
        }
        "NW" => {
            let kind = interaction(g(1));
            if g(0) == "1" {
                if g(2).is_empty() {
                    format!("opened {kind}")
                } else {
                    format!("opened {kind} with {}", label('n', g(2), names, "npc"))
                }
            } else {
                format!("closed {kind} after {}s", g(2))
            }
        }
        "GS" => format!(
            "{} offers {} quests, takes {} ({} options)",
            label('n', g(0), names, "npc"),
            list_len(g(1)),
            list_len(g(2)),
            list_len(g(3))
        ),
        "GO" => {
            let mut s = format!(
                "{}: {}",
                label('n', g(0), names, "npc"),
                label('o', g(1), names, "option")
            );
            if g(4) == "1" {
                s.push_str(" (confirmed)");
            }
            s
        }
        "GN" => format!("{} ({})", g(1), g(0)),
        "GC" => {
            if g(0) == "1" {
                format!(
                    "confirm {} costs {}",
                    label('o', g(1), names, "option"),
                    money(g(2)).trim_start_matches('+')
                )
            } else {
                "confirmation cancelled".into()
            }
        }
        "PC" => match g(0) {
            "1" => format!("choice {} offered ({} options)", g(1), g(2)),
            "2" => format!("choice {}: answered {}", g(1), g(2)),
            _ => format!("choice {} closed after {}s", g(1), g(2)),
        },
        "IR" => {
            if g(0).is_empty() {
                "read a text".into()
            } else {
                format!("read {}", g(0))
            }
        }
        "CK" => {
            let kind = match g(0) {
                "0" => "appearance",
                "1" => "mount",
                "2" => "achievement",
                "3" => "decor",
                other => other,
            };
            let what = if g(2) == "1" {
                "tracking"
            } else {
                "stopped tracking"
            };
            let base = if g(3) == "s" { " (baseline)" } else { "" };
            format!("{what} {kind} {}{base}", g(1))
        }
        "VB" => {
            let what = if g(0) == "r" { "bought back" } else { "bought" };
            let mut s = format!("{what} {} x{} for {}", item(1), g(2), coin(g(3)));
            if !g(4).is_empty() {
                s.push_str(&format!(" from {}", label('n', g(4), names, "npc")));
            }
            if g(5) == "1" {
                s.push_str(" (plus other cost)");
            }
            s
        }
        "AU" => {
            let thing = if g(2).is_empty() {
                "an item".to_string()
            } else {
                item(2)
            };
            let n = if g(3).is_empty() {
                String::new()
            } else {
                format!(" x{}", g(3))
            };
            match g(0) {
                "p" if g(7) == "1" => format!("posted {thing}{n} at {} each", coin(g(5))),
                "p" => {
                    let prices = parts(&[
                        if g(4).is_empty() {
                            String::new()
                        } else {
                            format!("bid {}", coin(g(4)))
                        },
                        if g(5).is_empty() {
                            String::new()
                        } else {
                            format!("buyout {}", coin(g(5)))
                        },
                    ]);
                    if prices.is_empty() {
                        format!("posted {thing}{n}")
                    } else {
                        format!("posted {thing}{n}: {prices}")
                    }
                }
                "b" => format!("bid {} on {thing}", coin(g(4))),
                "y" if g(7) == "1" => {
                    format!("bought {thing}{n} for {} ({} each)", coin(g(4)), coin(g(5)))
                }
                "y" => format!("bought out {thing} for {}", coin(g(4))),
                "x" => format!("cancelled {thing}{n}"),
                "s" => format!("sold {thing}{n}"),
                "e" => format!("expired {thing}{n}"),
                "o" => format!("outbid on {thing}"),
                "r" => format!("auction removed: {thing}{n}"),
                _ => f.join("\t"),
            }
        }
        "ML" => {
            let who = g(1);
            match g(0) {
                "s" => {
                    let extras = parts(&[
                        if g(2).is_empty() || g(2) == "0" {
                            String::new()
                        } else {
                            coin(g(2))
                        },
                        if g(3).is_empty() || g(3) == "0" {
                            String::new()
                        } else {
                            format!("COD {}", coin(g(3)))
                        },
                        item_stacks(g(4), names),
                    ]);
                    let postage = if g(5).is_empty() || g(5) == "0" {
                        String::new()
                    } else {
                        format!(" (postage {})", coin(g(5)))
                    };
                    if extras.is_empty() {
                        format!("sent to {who}{postage}")
                    } else {
                        format!("sent to {who}: {extras}{postage}")
                    }
                }
                "m" => format!("took {} from {who}", coin(g(2))),
                "i" => {
                    let cod = if g(3).is_empty() || g(3) == "0" {
                        String::new()
                    } else {
                        format!(" (COD {})", coin(g(3)))
                    };
                    format!("took {} from {who}{cod}", item_stacks(g(4), names))
                }
                "r" => format!("returned mail from {who}"),
                _ => f.join("\t"),
            }
        }
        "TR" => {
            if g(0) == "1" {
                format!("opened with {}", g(1))
            } else if g(2) == "1" || g(2) == "2" {
                let side = |money: &str, items: &str| {
                    let s = parts(&[
                        if money == "0" {
                            String::new()
                        } else {
                            coin(money)
                        },
                        item_stacks(items, names),
                    ]);
                    if s.is_empty() {
                        "nothing".to_string()
                    } else {
                        s
                    }
                };
                let how = if g(2) == "2" {
                    "accepted (not confirmed)"
                } else {
                    "completed"
                };
                format!(
                    "{how} with {}: gave {}; got {}",
                    g(1),
                    side(g(3), g(5)),
                    side(g(4), g(6))
                )
            } else {
                format!("cancelled with {}", g(1))
            }
        }
        "BK" => match g(0) {
            "d" => format!("deposited {} to {}", coin(g(2)), bank_type(g(1))),
            "w" => format!("withdrew {} from {}", coin(g(2)), bank_type(g(1))),
            "a" => format!("{} holds {}", bank_type(g(1)), coin(g(2))),
            _ => format!("{} now has {} tabs", bank_type(g(1)), g(2)),
        },
        "BG" => {
            let why = match g(2) {
                "s" => " (baseline)",
                "t" => " (bag slots changed)",
                "f" => " (full)",
                "o" => " (space again)",
                "w" => " (an item did not fit)",
                _ => "",
            };
            format!("{} of {} slots free{why}", g(0), g(1))
        }
        "EN" => format!(
            "slot {}: {} enchant {} gems [{}] (was enchant {} gems [{}])",
            g(0),
            item(1),
            g(2),
            item_ids(g(3), names),
            g(4),
            item_ids(g(5), names)
        ),
        "WE" => {
            if g(1) == "0" {
                format!("slot {}: ended", g(0))
            } else {
                let mut s = format!("slot {}: enchant {}", g(0), g(1));
                if !g(2).is_empty() {
                    s.push_str(&format!(", {} charges", g(2)));
                }
                if !g(3).is_empty() {
                    s.push_str(&format!(", {}s left", g(3)));
                }
                s
            }
        }
        "IU" => format!(
            "{} from upgrade {}/{} (+{}), item level {}",
            item(0),
            g(1),
            g(2),
            g(3),
            g(4)
        ),
        "RW" => {
            let how = roll_type(g(2));
            match g(0) {
                "o" => format!("offered {}", item(1)),
                "w" => format!("won {} x{} ({how} {})", item(1), g(4), g(3)),
                _ => format!("lost {} ({how} {}, winner {})", item(1), g(3), g(4)),
            }
        }
        "LM" => {
            let mut s = loot_method(g(0)).to_string();
            if !g(2).is_empty() {
                s.push_str(&format!(" {}", g(2)));
            }
            if !g(1).is_empty() {
                s.push_str(&format!(", {} and above", quality_word(g(1))));
            }
            if g(0) != "solo" && !g(3).is_empty() {
                s.push_str(&format!(", group of {}", g(3)));
            }
            s.push_str(if g(4) == "s" {
                " (at login)"
            } else {
                " (changed)"
            });
            s
        }
        "LT" => {
            let how = if g(0) == "p" { "was handed" } else { "looted" };
            format!("{} {how} {} x{}", g(1), item(2), g(3))
        }
        "LD" => {
            let how = roll_type(g(3));
            let on = item(2);
            match g(0) {
                "r" => format!("{} rolled {} ({how}) on {on}", g(1), g(4)),
                "s" => format!("{} chose {how} on {on}", g(1)),
                "p" => format!("{} passed on {on}", g(1)),
                "w" if g(4).is_empty() => format!("{} won {on}", g(1)),
                "w" => format!("{} won {on} ({how} {})", g(1), g(4)),
                _ => format!("everyone passed on {on}"),
            }
        }
        "IQ" => format!("{} is {}", item(0), quality_word(g(1))),
        "TN" => {
            let state = match g(2) {
                "0" => "you are here",
                "1" => "reachable",
                "2" => "unreachable",
                _ => "",
            };
            parts(&[format!("{} ({})", g(1), g(0)), state.to_string()])
        }
        "TF" => {
            let mut s = format!("to {}", label('t', g(0), names, "flight point"));
            if !g(1).is_empty() {
                s.push_str(&format!(
                    " from {}",
                    label('t', g(1), names, "flight point")
                ));
            }
            if !g(2).is_empty() && g(2) != "0" {
                s.push_str(&format!(" for {}", coin(g(2))));
            }
            s
        }
        "MO" => {
            let what = if g(1).is_empty() {
                "a mount".to_string()
            } else {
                spell(1)
            };
            if g(0) == "1" {
                let mut s = format!("mounted {what}");
                if g(3) == "1" {
                    s.push_str(", flying allowed here");
                }
                if g(4) == "s" {
                    s.push_str(" (already on)");
                }
                s
            } else {
                format!("dismounted {what} after {}s", g(3))
            }
        }
        "WP" => {
            if g(0) == "1" {
                let late = if g(4) == "s" { " (already set)" } else { "" };
                format!("set on map {} at {}, {}{late}", g(1), g(2), g(3))
            } else {
                "cleared".into()
            }
        }
        "SM" => match g(0) {
            "o" => {
                let place = if g(2).is_empty() {
                    String::new()
                } else {
                    format!(" to {}", g(2))
                };
                let answer = if g(3).is_empty() {
                    String::new()
                } else {
                    format!(", {}s to answer", g(3))
                };
                let skip = if g(4) == "1" {
                    ", skips the start experience"
                } else {
                    ""
                };
                format!("summon offered{place} (reason {}){answer}{skip}", g(1))
            }
            "a" => "summon accepted".into(),
            _ => "summon cancelled".into(),
        },
        "TM" => {
            let what = match g(1) {
                "pvp" => "battleground start timer",
                "challenge" => "challenge countdown",
                "countdown" => "pull timer",
                "plunder" => "Plunderstorm countdown",
                "world" => "world timer",
                _ => "timer",
            };
            let id = if g(4).is_empty() {
                String::new()
            } else {
                format!(" {}", g(4))
            };
            if g(0) == "1" {
                let secs = if g(2).is_empty() {
                    String::new()
                } else {
                    format!(": {}s of {}s", g(2), g(3))
                };
                format!("{what}{id} started{secs}")
            } else {
                format!("{what}{id} ended")
            }
        }
        "IB" => {
            if g(0) == "1" {
                format!("removal from the instance in {}s", g(1))
            } else {
                "removal from the instance stopped".into()
            }
        }
        "DF" => {
            let late = if g(3) == "s" { " (at login)" } else { "" };
            format!(
                "dungeon {}, raid {}, legacy raid {}{late}",
                difficulty_name(g(0)),
                difficulty_name(g(1)),
                difficulty_name(g(2))
            )
        }
        "VG" => {
            let kind = if g(0) == "d" { "died" } else { "in range" };
            let what = match g(2) {
                "1" => "PvP bounty",
                "3" => "treasure",
                _ => "",
            };
            let npc = if g(3).is_empty() {
                String::new()
            } else {
                format!("npc {}", g(3))
            };
            let at = if g(5).is_empty() {
                String::new()
            } else {
                format!("map {} at {}, {}", g(4), g(5), g(6))
            };
            parts(&[
                format!("{} ({}) {kind}", g(7), g(1)),
                what.to_string(),
                npc,
                at,
            ])
        }
        "AI" => {
            let left = if g(2).is_empty() {
                String::new()
            } else {
                format!("{}s left", g(2))
            };
            let now = if g(3) == "1" { "running now" } else { "" };
            parts(&[
                format!("{} ({}) on map {}", g(4), g(0), g(1)),
                left,
                now.to_string(),
            ])
        }
        "MP" => {
            let on = parts(
                &g(0)
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(|s| {
                        if s.bytes().all(|b| b.is_ascii_digit()) {
                            label('s', s, names, "spell")
                        } else {
                            s.to_string()
                        }
                    })
                    .collect::<Vec<_>>(),
            );
            let late = if g(1) == "s" { " (at login)" } else { "" };
            if on.is_empty() {
                format!("none{late}")
            } else {
                format!("{on}{late}")
            }
        }
        "SE" => match g(0) {
            "s" => format!(
                "{} ({}) stage {} of {}{}",
                g(4),
                g(1),
                g(2),
                g(3),
                if g(5).is_empty() {
                    String::new()
                } else {
                    format!(" in {}", g(5))
                }
            ),
            "l" => format!("left scenario {}", g(1)),
            "c" => format!("completed scenario {}, {} xp, {}", g(1), g(2), coin(g(3))),
            "p" => {
                let text = if g(6).is_empty() {
                    String::new()
                } else {
                    format!("{}: ", g(6))
                };
                let done = if g(5) == "1" { " done" } else { "" };
                format!("{text}criterion {} {}/{}{done}", g(2), g(3), g(4))
            }
            _ => f.join("\t"),
        },
        "BR" => {
            let what = if g(0) == "item" {
                item(1)
            } else {
                format!("{} {}", g(0), g(1))
            };
            format!("{what} x{}", g(2))
        }
        "TK" => match g(0) {
            "p" => format!("market price {}", coin(g(1))),
            "b" => format!("buy result {}", g(1)),
            "s" => format!("sell result {}", g(1)),
            _ => "sold".into(),
        },
        "LP" => {
            if g(0) == "0" {
                "current spec".into()
            } else {
                format!("spec {}", g(0))
            }
        }
        "DI" => format!("{} x{}", item(0), g(1)),
        "RF" => {
            let mut s = format!(
                "{}: {}, standing {}",
                label('f', g(0), names, "faction"),
                reaction(g(1)),
                g(2)
            );
            if !g(3).is_empty() || !g(4).is_empty() {
                s.push_str(&format!(" (tier {} to {})", g(3), g(4)));
            }
            if g(5) == "1" {
                s.push_str(", at war");
            }
            if g(6) == "1" {
                s.push_str(", account-wide");
            }
            if g(7) == "s" {
                s.push_str(" (baseline)");
            }
            s
        }
        "RG" => {
            let mut s = format!(
                "{}: paragon {}/{}, level {}",
                label('f', g(0), names, "faction"),
                g(1),
                g(2),
                g(4)
            );
            if g(3) == "1" {
                s.push_str(", reward pending");
            }
            if g(5) == "s" {
                s.push_str(" (baseline)");
            }
            s
        }
        "RV" => {
            let faction = label('f', g(1), names, "faction");
            match g(0) {
                "b" => format!(
                    "{faction}: renown {} of {}, {}/{} to the next (baseline)",
                    g(2),
                    g(3),
                    g(4),
                    g(5)
                ),
                "l" => format!("{faction}: renown {} to {}", g(3), g(2)),
                "u" => format!("{faction}: unlocked"),
                other => format!("{other} {faction}"),
            }
        }
        "RR" => {
            let mut parts: Vec<String> = Vec::new();
            if !g(2).is_empty() {
                parts.push(item(2));
            }
            if !g(3).is_empty() {
                parts.push(spell(3));
            }
            if !g(4).is_empty() {
                parts.push(format!("mount {}", g(4)));
            }
            if !g(5).is_empty() {
                parts.push(format!("transmog set {}", g(5)));
            }
            if !g(6).is_empty() {
                parts.push(format!("title {}", g(6)));
            }
            format!(
                "{} renown {}: {}",
                label('f', g(0), names, "faction"),
                g(1),
                parts.join(", ")
            )
        }
        "AD" => {
            let mut s = label('a', g(0), names, "achievement");
            if g(2) == "1" {
                s.push_str(", guild");
            }
            if g(1) == "1" {
                s.push_str(", already earned");
            }
            if !g(3).is_empty() {
                s.push_str(&format!(", category {}", g(3)));
            }
            if !g(4).is_empty() {
                s.push_str(&format!(", reward: {}", g(4)));
            }
            s
        }
        "AT" => {
            let ach = label('a', g(0), names, "achievement");
            let what = if g(5).is_empty() {
                format!("criterion {}", g(1))
            } else {
                g(5).to_string()
            };
            let mut s = format!("{ach}: {what} {}/{}", g(2), g(3));
            if g(4) == "1" {
                s.push_str(", done");
            }
            if !g(6).is_empty() {
                s.push_str(&format!(", timed {}s", g(6)));
            }
            s
        }
        "BQ" => {
            let map = if g(2).is_empty() {
                "a battleground"
            } else {
                g(2)
            };
            let mut s = match g(0) {
                "q" => format!("queued for {map}"),
                "c" => format!("{map} is ready"),
                "a" => format!("entered {map}"),
                _ => {
                    let was = match g(8) {
                        "q" => "left the queue for",
                        "c" => "did not enter",
                        "a" => "left",
                        _ => "queue ended for",
                    };
                    format!("{was} {map}")
                }
            };
            if !g(3).is_empty() && g(3) != "0" {
                s.push_str(&format!(" ({}v{})", g(3), g(3)));
            }
            if g(4) == "1" {
                s.push_str(", rated");
            }
            if !g(6).is_empty() {
                s.push_str(&format!(", role {}", g(6)));
            }
            if !g(7).is_empty() {
                s.push_str(&format!(", {}s", g(7)));
            }
            if g(9) == "s" {
                s.push_str(" (baseline)");
            }
            s
        }
        "BM" => match g(0) {
            "1" => {
                let mut s = format!("in {}", if g(2).is_empty() { "a match" } else { g(2) });
                if !g(3).is_empty() {
                    s.push_str(&format!(" ({})", g(3)));
                }
                if g(6) == "1" {
                    s.push_str(", rated");
                }
                if g(8) == "1" && !g(9).is_empty() {
                    s.push_str(&format!(", team {}", g(9)));
                } else if !g(9).is_empty() {
                    s.push_str(&format!(", {}", side(g(9))));
                }
                if g(5) == "s" {
                    s.push_str(" (baseline)");
                }
                s
            }
            "2" => "started".into(),
            "3" => format!("left after {}s", g(2)),
            _ => {
                let result = match g(5) {
                    "w" => "you won",
                    "l" => "you lost",
                    "d" => "draw",
                    _ => "ended",
                };
                let winner = if g(7) == "1" {
                    format!("team {}", g(2))
                } else {
                    side(g(2)).to_string()
                };
                let mut s = format!("{result}, winner {winner}");
                if !g(3).is_empty() {
                    s.push_str(&format!(", duration {}", g(3)));
                }
                if g(6) == "0" {
                    s.push_str(", rating not affected");
                }
                s
            }
        },
        "BS" => {
            if g(1).is_empty() && g(0) == "?" {
                "no row from the client".into()
            } else {
                let mut s = format!(
                    "{} kills, {} honorable, {} deaths, {} honor, {} damage, {} healing",
                    g(0),
                    g(1),
                    g(2),
                    g(3),
                    g(4),
                    g(5)
                );
                if !g(6).is_empty() && g(6) != "0" {
                    s.push_str(&format!(", rating {} ({})", g(6), g(7)));
                }
                if !g(14).is_empty() {
                    s.push_str(&format!(", stats {}", g(14)));
                }
                s
            }
        }
        "HL" => {
            if g(1) == "1" {
                format!("honor level {} (up)", g(0))
            } else {
                format!("honor level {}", g(0))
            }
        }
        "PB" => {
            let mut s = format!("honor level {}", g(0));
            if !g(1).is_empty() || !g(2).is_empty() {
                s.push_str(&format!(
                    ", war mode {}{}",
                    if g(1) == "1" { "on" } else { "off" },
                    if g(2) == "1" { " (active)" } else { "" }
                ));
            }
            if g(3) == "s" {
                s.push_str(" (baseline)");
            }
            s
        }
        "ZP" => {
            let mut s = if g(0).is_empty() {
                "no PvP rule".to_string()
            } else {
                g(0).to_string()
            };
            if g(1) == "1" {
                s.push_str(" (subzone)");
            }
            if !g(2).is_empty() {
                s.push_str(&format!(", {}", g(2)));
            }
            s
        }
        "K" => {
            let label = label('n', g(0), names, "npc");
            let by = match g(1) {
                "1" => "you",
                "0" => "party",
                "2" => "you or your pet (no killing-blow report)",
                _ => "unknown",
            };
            format!("{label} by {by}")
        }
        "N" => format!("{} ({})", g(1), g(0)),
        "F" => {
            if g(0) == "1" {
                "started".into()
            } else {
                format!("ended after {}s", g(1))
            }
        }
        "E" => {
            if g(0) == "1" {
                format!("started {} ({})", g(4), g(1))
            } else {
                format!(
                    "ended {} ({}) after {}s",
                    g(1),
                    if g(2) == "1" { "kill" } else { "wipe" },
                    g(3)
                )
            }
        }
        "B" => format!("{} ({})", g(1), g(0)),
        "L" => g(0).to_string(),
        "X" => {
            let mut s = format!("{} xp", g(0));
            match g(3) {
                "k" => s.push_str(&format!(" from {}", label('n', g(4), names, "npc"))),
                "q" => s.push_str(&format!(" from {}", label('q', g(4), names, "quest"))),
                "e" => s.push_str(" from exploring"),
                _ => {}
            }
            if !g(1).is_empty() && !g(2).is_empty() {
                s.push_str(&format!(" ({}/{})", g(1), g(2)));
            }
            s
        }
        "M" => {
            if g(1).is_empty() || g(1) == "0" {
                money(g(0))
            } else {
                format!(
                    "{} ({} mailed between your characters)",
                    money(g(0)),
                    money(g(1))
                )
            }
        }
        "O" => format!("{} x{}{}", item(0), g(1), variant(g(2), g(3), g(4))),
        "II" => {
            let mut s = item(0);
            if !g(1).is_empty() {
                s.push_str(&format!(", class {}/{}", g(1), g(2)));
            }
            if !g(3).is_empty() {
                s.push_str(&format!(
                    ", {}",
                    g(3).trim_start_matches("INVTYPE_").to_lowercase()
                ));
            }
            if !g(4).is_empty() && g(4) != "0" {
                s.push_str(&format!(", item level {}", g(4)));
            }
            if !g(5).is_empty() && g(5) != "0" {
                s.push_str(&format!(", needs level {}", g(5)));
            }
            if !g(6).is_empty() && g(6) != "0" {
                s.push_str(&format!(", sells for {}", coin(g(6))));
            }
            if !g(7).is_empty() && g(7) != "0" {
                s.push_str(&format!(", {}", bind_word(g(7))));
            }
            if !g(8).is_empty() && g(8) != "1" {
                s.push_str(&format!(", stacks to {}", g(8)));
            }
            s
        }
        "IT" => format!("{}: {}", item(0), g(1)),
        "Q" => {
            let quest = label('q', g(1), names, "quest");
            match g(0) {
                "1" => format!("accepted {quest}"),
                "3" => format!("abandoned or failed {quest}"),
                _ => format!("turned in {quest} ({} xp, {})", g(2), money(g(3))),
            }
        }
        "G" => {
            if g(1) == "0" {
                format!("slot {}: emptied", g(0))
            } else {
                format!("slot {}: {}{}", g(0), item(1), variant(g(2), g(3), g(4)))
            }
        }
        "A" => {
            let kind = match g(2) {
                "1" => "buff",
                "0" => "debuff",
                _ => "aura",
            };
            if g(0) == "1" {
                let mut s = format!("+{kind} {}", spell(1));
                if let Ok(d) = g(3).parse::<f64>()
                    && d > 0.0
                {
                    s.push_str(&format!(" for {d}s"));
                }
                match g(4) {
                    "1" => s.push_str(" from you"),
                    "0" => s.push_str(" from someone else"),
                    _ => {}
                }
                if g(5) == "s" {
                    s.push_str(" (already on)");
                }
                s
            } else {
                format!("-aura {} after {}s", spell(1), g(2))
            }
        }
        "CP" => {
            if g(0) == "1" {
                let cont = if g(2) == "1" { " (continued)" } else { "" };
                format!("arrived: {}{cont}", spell(1))
            } else {
                format!("left after {}s", g(1))
            }
        }
        "RS" => if g(0) == "1" { "started" } else { "stopped" }.into(),
        "I" => {
            let place = match g(2) {
                "v" => " at vendor",
                "b" => " at bank",
                "m" => " by mail",
                "g" => " at guild bank",
                "t" => " by trade",
                "a" => " at the auction house",
                "l" => " from loot",
                "c" => " while crafting",
                _ => "",
            };
            let n = g(1);
            let n = if n.starts_with('-') {
                n.to_string()
            } else {
                format!("+{n}")
            };
            let mut s = format!("{n} {}{place}", item(0));
            if g(2) == "v" && !g(3).is_empty() {
                s.push_str(&format!(" ({})", label('n', g(3), names, "npc")));
            }
            if g(2) == "v" && !g(4).is_empty() && g(4) != "0" {
                s.push_str(&format!(", sells for {} each", coin(g(4))));
            }
            s
        }
        "SN" | "IN" => String::new(),
        "TG" => {
            let mut s = format!("talent group {}", g(1));
            if !g(2).is_empty() {
                s.push_str(&format!(" of {}", g(2)));
            }
            if !g(3).is_empty() {
                s.push_str(&format!(
                    ", spec {}",
                    if g(4).is_empty() { g(3) } else { g(4) }
                ));
            }
            let tabs = parts(
                &g(6)
                    .split(',')
                    .filter(|t| !t.is_empty())
                    .map(|t| {
                        let mut it = t.split(':');
                        let id = it.next().unwrap_or("");
                        let spent = it.next().unwrap_or("");
                        let avail = it.next().unwrap_or("");
                        let mut out = format!("{} {spent} spent", label('g', id, names, "tab"));
                        if !avail.is_empty() {
                            out.push_str(&format!(" ({avail} available)"));
                        }
                        out
                    })
                    .collect::<Vec<_>>(),
            );
            if !tabs.is_empty() {
                s.push_str(&format!(": {tabs}"));
            }
            if !g(7).is_empty() || !g(8).is_empty() {
                s.push_str(&format!(", unspent {} class, {} spec", g(7), g(8)));
            }
            match g(0) {
                "s" => s.push_str(" (baseline)"),
                "w" => s.push_str(" (group changed)"),
                "r" => s.push_str(" (reset by the client)"),
                _ => {}
            }
            if !g(10).is_empty() {
                s.push_str(&format!(", loadout {}", g(10)));
            }
            s
        }
        "TA" => {
            let what = if g(6).is_empty() {
                format!("node {}", g(1))
            } else {
                spell(6)
            };
            let mut s = format!("group {}: {what} rank {}", g(0), g(3));
            if !g(5).is_empty() {
                s.push_str(&format!("/{}", g(5)));
            }
            if !g(4).is_empty() {
                s.push_str(&format!(" (was {})", g(4)));
            }
            if !g(7).is_empty() {
                s.push_str(&format!(" in {}", label('g', g(7), names, "tab")));
            }
            if g(8) == "s" {
                s.push_str(" (baseline)");
            }
            s
        }
        "TY" => format!("{} ({})", g(1), g(0)),
        "TV" => format!(
            "tree {} for class {}: {} nodes, layout {}",
            g(0),
            g(1),
            g(2),
            g(3)
        ),
        "TD" => {
            let mut s = format!("tree {} node {} at {}, {}", g(0), g(1), g(2), g(3));
            if !g(6).is_empty() {
                s.push_str(&format!(" in {}", label('g', g(6), names, "tab")));
            }
            let entries = g(7).split(',').filter(|e| !e.is_empty()).count();
            let edges = g(8).split(',').filter(|e| !e.is_empty()).count();
            s.push_str(&format!(
                " ({} ranks, {entries} entries, {edges} edges)",
                g(5)
            ));
            s
        }
        "TQ" => match g(0) {
            "p" => format!("respec offered for {} (type {})", coin(g(1)), g(2)),
            "r" => if g(1) == "1" {
                "pet talents reset by the client"
            } else {
                "talents reset by the client"
            }
            .into(),
            "i" => if g(1) == "1" {
                "pet spec changed by the client"
            } else {
                "spec changed by the client"
            }
            .into(),
            _ => f.join("\t"),
        },
        "TI" => {
            let base = if g(1).is_empty() {
                "no title".to_string()
            } else {
                format!("{} (title {})", g(1), g(0))
            };
            if g(2) == "s" {
                format!("{base} (baseline)")
            } else {
                base
            }
        }
        "EQ" => {
            let set = if g(2).is_empty() {
                format!("set {}", g(1))
            } else {
                format!("{} ({})", g(2), g(1))
            };
            match g(0) {
                "w" => format!(
                    "swapped to {set}{}",
                    if g(6) == "0" { " (failed)" } else { "" }
                ),
                "x" => format!("deleted {set}"),
                kind => {
                    let mut s = format!(
                        "{} {set}: {} items",
                        if kind == "b" { "found" } else { "changed" },
                        g(3)
                    );
                    if !g(4).is_empty() {
                        s.push_str(&format!(", spec {}", g(4)));
                    }
                    if !g(5).is_empty() && g(5) != "0" {
                        s.push_str(&format!(", {} slots ignored", g(5)));
                    }
                    s
                }
            }
        }
        "PD" => {
            let mut s = format!("{} level {}", g(0), g(1));
            if !g(5).is_empty() {
                s.push_str(&format!(", {}", g(5)));
            }
            match g(4) {
                "1" => s.push_str(", unhappy"),
                "2" => s.push_str(", content"),
                "3" => s.push_str(", happy"),
                "" => {}
                other => s.push_str(&format!(", happiness {other}")),
            }
            if !g(6).is_empty() {
                s.push_str(&format!(", training points {}/{}", g(7), g(6)));
            }
            if !g(2).is_empty() {
                s.push_str(&format!(", {}/{} xp", g(2), g(3)));
            }
            if g(8) == "s" {
                s.push_str(" (summoned or found)");
            }
            s
        }
        "XD" => format!(
            "experience gain {}{}",
            if g(0) == "1" { "off" } else { "on" },
            if g(1) == "s" { " (baseline)" } else { "" }
        ),
        "BA" => match g(0) {
            "a" => {
                let n = list_len(g(3));
                if n > 0 {
                    format!(
                        "appearance applied{} ({n} options: {})",
                        barber_price(g(2)),
                        g(3)
                    )
                } else {
                    format!("appearance applied{}", barber_price(g(2)))
                }
            }
            _ => format!(
                "purchase {}{}",
                if g(1) == "1" { "succeeded" } else { "failed" },
                barber_price(g(2))
            ),
        },
        "GA" => format!(
            "game mode {}{}",
            g(0),
            if g(1) == "s" { " (baseline)" } else { "" }
        ),
        "CD" => {
            let sex = match g(0) {
                "2" => "male",
                "3" => "female",
                "1" => "unknown sex",
                other => other,
            };
            let mut s = format!("{sex}, class {}, race {}", g(1), g(2));
            if !g(3).is_empty() {
                s.push_str(&format!(", effective level {}", g(3)));
            }
            if g(4) == "s" {
                s.push_str(" (baseline)");
            }
            s
        }
        "SX" => {
            let why = match g(0) {
                "s" => "session baseline",
                "g" => "gear",
                "l" => "level",
                "t" => "talents",
                "o" => "other",
                other => other,
            };
            format!("{} {} ({})", g(1), g(2), why)
        }
        "SB" => {
            let what = match g(0) {
                "b" => "baseline",
                "a" => "added",
                "x" => "removed",
                other => other,
            };
            let book = if g(1) == "t" { "pet" } else { "book" };
            format!("{what} {book} {}: {}", g(2), spell_ranks(g(3), names))
        }
        "AB" => {
            let mut s = format!("slot {}: {}", g(0), bar_entry(g(1), g(2), names));
            if !g(3).is_empty() {
                s.push_str(&format!(" (was {})", bar_entry(g(3), g(4), names)));
            }
            if g(5) == "1" {
                s.push_str(", not the highest rank");
            }
            match g(6) {
                "s" => s.push_str(" (baseline)"),
                "p" => s.push_str(" (put there by the client)"),
                _ => {}
            }
            s
        }
        "TB" => {
            let mut s = g(2).to_string();
            if !g(3).is_empty() {
                s.push_str(&format!(" ({})", g(3)));
            }
            if !g(4).is_empty() && g(4) != "0" {
                s.push_str(&format!(" for {}", coin(g(4))));
            }
            if !g(0).is_empty() {
                s.push_str(&format!(" from {}", label('n', g(0), names, "npc")));
            }
            if !g(5).is_empty() && g(5) != "0" {
                s.push_str(&format!(", needs level {}", g(5)));
            }
            s
        }
        "TC" => {
            let mut s = g(2).to_string();
            if !g(3).is_empty() {
                s.push_str(&format!(" ({})", g(3)));
            }
            let state = match g(6) {
                "a" => "available",
                "u" => "unavailable",
                "k" => "known",
                other => other,
            };
            s.push_str(&format!(", {state}"));
            if !g(4).is_empty() && g(4) != "0" {
                s.push_str(&format!(", {}", coin(g(4))));
            }
            if !g(5).is_empty() && g(5) != "0" {
                s.push_str(&format!(", needs level {}", g(5)));
            }
            if !g(8).is_empty() {
                s.push_str(&format!(", needs skill {}", g(8).replace(':', " ")));
            }
            if !g(9).is_empty() {
                s.push_str(&format!(", needs {}", g(9)));
            }
            if !g(0).is_empty() {
                s.push_str(&format!(" at {}", label('n', g(0), names, "npc")));
            }
            s
        }
        "SG" => {
            let mut s = format!("{} slots, {} pets stabled", g(1), g(2));
            if !g(3).is_empty() {
                s.push_str(&format!(", next slot {}", coin(g(3))));
            }
            let pets: Vec<String> = g(4)
                .split(',')
                .filter(|p| !p.is_empty())
                .map(|p| {
                    let mut it = p.splitn(4, ':');
                    let (slot, level, family, name) = (
                        it.next().unwrap_or(""),
                        it.next().unwrap_or(""),
                        it.next().unwrap_or(""),
                        it.next().unwrap_or(""),
                    );
                    format!("{name} (level {level} {family}, slot {slot})")
                })
                .collect();
            if !pets.is_empty() {
                s.push_str(&format!(": {}", pets.join(", ")));
            }
            if g(0) == "s" {
                s.push_str(" (first read)");
            }
            s
        }
        "AR" => {
            let what = if g(1).is_empty() {
                "auto repeat".to_string()
            } else {
                spell(1)
            };
            if g(0) == "1" {
                format!("{what} on")
            } else {
                format!("{what} off after {}s", g(2))
            }
        }
        "SO" => format!(
            "{} {}",
            spell(1),
            if g(0) == "1" {
                "proc started"
            } else {
                "proc ended"
            }
        ),
        "EC" => {
            let on = if g(1).is_empty() {
                String::new()
            } else {
                format!(" on {}", item(1))
            };
            format!("{}{on}", if g(0) == "1" { "succeeded" } else { "failed" })
        }
        "CL" => {
            let mut s = format!("{} {}", g(0), g(1));
            if !g(2).is_empty() {
                s.push_str(&format!(" of {}", g(2)));
            }
            if g(3) == "c" && !g(4).is_empty() {
                s.push_str(&format!(", was {}", g(4)));
            }
            let detail = g(5);
            if let Some(rest) = detail.strip_prefix("dr") {
                let mut it = rest.splitn(2, ':');
                let unlocked = it.next().unwrap_or("");
                let owned = it.next().unwrap_or("");
                if unlocked == "1" {
                    s.push_str(", skyriding unlocked");
                }
                if !owned.is_empty() && owned != "0" {
                    s.push_str(&format!(", {owned} skyriding mounts"));
                }
            } else if !detail.is_empty() {
                let cats: Vec<String> = detail
                    .split(',')
                    .filter_map(|c| c.split_once(':'))
                    .map(|(cat, n)| format!("{} {n}", transmog_category(cat)))
                    .collect();
                s.push_str(&format!(": {}", cats.join(", ")));
            }
            if g(3) == "s" {
                s.push_str(" (first read)");
            }
            s
        }
        "CA" => {
            let what = if !g(2).is_empty() {
                g(2).to_string()
            } else if !g(5).is_empty() {
                item(5)
            } else if !g(1).is_empty() {
                format!("#{}", g(1))
            } else {
                String::new()
            };
            let mut s = format!("{} {what}", g(0)).trim_end().to_string();
            match g(0) {
                "pet" => {
                    if !g(3).is_empty() {
                        s.push_str(&format!(", {}", pet_type(g(3))));
                    }
                    if !g(6).is_empty() {
                        s.push_str(&format!(", level {}", g(6)));
                    }
                }
                "heirloom" if g(6) == "upgrade" => s.push_str(" upgraded"),
                "mountequip" => {
                    s = format!("mount equipment {}", item(1));
                    if g(6) == "0" {
                        s.push_str(" failed");
                    }
                }
                "appearance" | "cosmetic" if !g(6).is_empty() => {
                    s.push_str(&format!(" ({})", transmog_category(g(6))));
                }
                "battleslot" => s = "pet battle slot unlocked".into(),
                _ => {}
            }
            if !g(4).is_empty() {
                s.push_str(&format!(" [{}]", g(4)));
            }
            s
        }
        "CX" => match g(0) {
            "pet" if !g(1).is_empty() => format!("pet released (species {})", g(1)),
            "pet" => "pet released".into(),
            other => format!("{other} unlearned"),
        },
        "SP" => {
            let name = if g(2).is_empty() {
                if g(1).is_empty() {
                    "a pet".to_string()
                } else {
                    format!("species {}", g(1))
                }
            } else {
                g(2).to_string()
            };
            let level = if g(3).is_empty() {
                String::new()
            } else {
                format!(" (level {})", g(3))
            };
            let custom = if g(4).is_empty() {
                String::new()
            } else {
                format!(" named {}", g(4))
            };
            match g(0) {
                "0" => format!("dismissed {name}{custom}"),
                "s" => format!("{name}{level}{custom} already out"),
                _ => format!("summoned {name}{level}{custom}"),
            }
        }
        "TX" => {
            let kind = if g(1) == "1" {
                "illusion"
            } else {
                "appearance"
            };
            let what = if g(4).is_empty() {
                if g(3).is_empty() || g(3) == "0" {
                    "cleared".to_string()
                } else {
                    format!("{kind} {}", g(3))
                }
            } else {
                item(4)
            };
            let mut s = format!("{}: {what}", outfit_slot(g(0)));
            if !g(5).is_empty() {
                s.push_str(&format!(", outfit {}", g(5)));
            }
            if !g(6).is_empty() {
                s.push_str(&format!(", cost {}", coin(g(6))));
                if g(7) == "1" {
                    s.push_str(" (discount available)");
                }
            }
            s
        }
        "TO" => {
            let name = if g(2).is_empty() {
                format!("outfit {}", g(1))
            } else {
                format!("{} ({})", g(2), g(1))
            };
            match g(0) {
                "a" => format!(
                    "showing {name}{}",
                    if g(3) == "s" { " (first read)" } else { "" }
                ),
                "n" => format!("new outfit {name}"),
                "s" => format!("new custom set {name}"),
                other => format!("{other} {name}"),
            }
        }
        "BT" => match g(0) {
            "s" => {
                let who = if g(2) == "1" {
                    "trainer"
                } else if g(1) == "1" {
                    "wild pet"
                } else {
                    "player"
                };
                let mut s = format!(
                    "began against a {who}: {} vs {}",
                    pet_team(g(3)),
                    pet_team(g(4))
                );
                if !g(5).is_empty() {
                    s.push_str(&format!(" ({})", g(5)));
                }
                s
            }
            "x" => format!("species {} gained {} xp", g(1), g(2)),
            "l" => format!("species {} reached level {}", g(1), g(2)),
            "e" => {
                let result = match g(3) {
                    "w" => ", won",
                    "l" => ", lost",
                    _ => "",
                };
                format!(
                    "ended after {}s, {} rounds{result}, {} xp",
                    g(1),
                    g(2),
                    g(4)
                )
            }
            other => format!("{other} {}", f.join(" ")),
        },
        "SK" => format!("{} rank {}", g(0), g(1)),
        "RP" => format!("{} {}", g(0), g(1)),
        "AC" => format!("achievement {}", g(0)),
        "GU" => {
            let guild = g(1);
            let mut extra = Vec::new();
            if !g(2).is_empty() {
                extra.push(format!("rank {}", g(2)));
            }
            if g(4) == "1" {
                extra.push("guild leader".to_string());
            } else if g(3) == "1" {
                extra.push("officer".to_string());
            }
            let extra = if extra.is_empty() {
                String::new()
            } else {
                format!(" ({})", extra.join(", "))
            };
            match g(0) {
                "s" if guild.is_empty() => "not in a guild".into(),
                "s" => format!("in {guild}{extra}"),
                "j" => format!("joined {guild}{extra}"),
                "l" => format!("left {guild}"),
                "k" => format!("{guild}{extra}"),
                "i" if g(5).is_empty() => format!("invited to {guild}"),
                "i" => format!("{} invited you to {guild}", g(5)),
                "x" => format!("invite to {guild} withdrawn"),
                "a" => format!("accepted the invite to {guild}"),
                "d" => format!("declined the invite to {guild}"),
                other => format!("{other} {}", f.join(" ")),
            }
        }
        "GH" => {
            let mut s = format!("{} members, {} online", g(0), g(1));
            if !g(2).is_empty() && g(2) != g(1) {
                s.push_str(&format!(" ({} with mobile)", g(2)));
            }
            s
        }
        "FR" => {
            let who = {
                let mut s = g(1).to_string();
                let mut extra = Vec::new();
                if !g(2).is_empty() {
                    extra.push(format!("level {}", g(2)));
                }
                if !g(3).is_empty() {
                    extra.push(g(3).to_lowercase());
                }
                if !extra.is_empty() {
                    s.push_str(&format!(" ({})", extra.join(" ")));
                }
                s
            };
            let first = if g(4) == "b" { ", first list" } else { "" };
            match g(0) {
                "a" => format!("added friend {who}{first}"),
                "r" => format!("removed friend {who}"),
                "n" => format!("{who} came online"),
                "f" => format!("{who} went offline"),
                "i" => format!("ignored {who}{first}"),
                "u" => format!("stopped ignoring {who}"),
                other => format!("{other} {}", f.join(" ")),
            }
        }
        "FC" => format!(
            "{} friends ({} online), {} ignored; Battle.net {} ({} online), {} invites, {} blocked",
            g(0),
            g(1),
            g(2),
            g(3),
            g(4),
            g(5),
            g(6)
        ),
        "GI" => {
            let who = g(1);
            let roles = role_words(g(2));
            match g(0) {
                "r" if roles.is_empty() => format!("{who} invited you"),
                "r" => format!("{who} invited you as {roles}"),
                "x" => format!("invite from {who} cancelled"),
                "a" => format!("accepted the invite from {who}"),
                "d" => format!("declined the invite from {who}"),
                "s" => format!("invited {who}"),
                "q" => format!("asked to join {who}'s group"),
                "k" => format!("removed {who} from the group"),
                "p" => format!("gave the lead to {who}"),
                "m" => format!("made {who} an assistant"),
                "u" => format!("removed {who} as assistant"),
                "c" => "converted the group to a raid".into(),
                "y" => "converted the group to a party".into(),
                "l" => "left the group".into(),
                "e" => format!(
                    "everyone is an assistant: {}",
                    if who == "1" { "on" } else { "off" }
                ),
                other => format!("{other} {}", f.join(" ")),
            }
        }
        "GL" => match g(0) {
            "m" => {
                let mut bits = Vec::new();
                let role = role_words(g(2));
                if !role.is_empty() {
                    bits.push(role);
                }
                if g(3) == "1" {
                    bits.push("leader".to_string());
                }
                if g(4) == "1" {
                    bits.push("assistant".to_string());
                }
                if !g(5).is_empty() {
                    bits.push(format!("group {}", g(5)));
                }
                if g(6) == "1" {
                    bits.push("guild mate".to_string());
                }
                if g(7) == "1" {
                    bits.push("friend".to_string());
                }
                if !g(8).is_empty() {
                    bits.push(format!("level {}", g(8)));
                }
                if !g(9).is_empty() {
                    bits.push(g(9).to_lowercase());
                }
                if !g(11).is_empty() {
                    bits.push(g(11).to_lowercase());
                }
                if !g(12).is_empty() {
                    bits.push(g(12).to_string());
                }
                if !g(13).is_empty() {
                    bits.push(format!("<{}>", g(13)));
                }
                if g(10) == "0" {
                    bits.push("offline".to_string());
                }
                format!("{}: {}", g(1), bits.join(", "))
            }
            "l" if g(2) == "1" => "you lead the group".into(),
            "l" => format!("{} leads the group", g(1)),
            "s" => format!(
                "{}{}{}{}",
                if g(1) == "1" { "raid" } else { "party" },
                if g(2) == "1" { ", cross-faction" } else { "" },
                if g(3) == "1" { ", full" } else { "" },
                if g(4) == "1" { ", guild group" } else { "" }
            ),
            "j" => format!(
                "group formed ({})",
                match g(1) {
                    "1" => "home party",
                    "2" => "instance group",
                    other => other,
                }
            ),
            "x" => format!(
                "group ended ({})",
                match g(1) {
                    "1" => "home party",
                    "2" => "instance group",
                    other => other,
                }
            ),
            "p" => format!("role poll started by {}", g(1)),
            "c" => {
                let by = if g(2).is_empty() {
                    String::new()
                } else {
                    format!(" by {}", g(2))
                };
                let was = role_words(g(3));
                let now = role_words(g(4));
                format!(
                    "{} role {} to {}{by}",
                    g(1),
                    if was.is_empty() {
                        "none".to_string()
                    } else {
                        was
                    },
                    if now.is_empty() {
                        "none".to_string()
                    } else {
                        now
                    }
                )
            }
            other => format!("{other} {}", f.join(" ")),
        },
        "RC" => match g(0) {
            "s" => format!("{} started a ready check ({}s)", g(1), g(2)),
            "r" => format!(
                "{} {} after {}s",
                g(1),
                if g(2) == "1" { "ready" } else { "not ready" },
                g(3)
            ),
            "e" => format!(
                "{} after {}s: {} ready, {} not ready",
                if g(1) == "1" { "replaced" } else { "ended" },
                g(2),
                g(3),
                g(4)
            ),
            other => format!("{other} {}", f.join(" ")),
        },
        "LF" => match g(0) {
            "b" => format!(
                "group finder {}, dungeon finder {}, premade groups {}, enabled {}{}",
                yes_no(g(1)),
                yes_no(g(2)),
                yes_no(g(3)),
                yes_no(g(4)),
                if g(5) == "e" { " (changed)" } else { "" }
            ),
            "l" | "u" => {
                let mut s = format!(
                    "{} {}",
                    if g(0) == "l" {
                        "listed"
                    } else {
                        "updated listing"
                    },
                    if g(2).is_empty() { g(1) } else { g(2) }
                );
                let mut extra = Vec::new();
                if g(3) == "1" {
                    extra.push("auto accept".to_string());
                }
                if g(4) == "1" {
                    extra.push("private".to_string());
                }
                if g(5) != "0" && !g(5).is_empty() {
                    extra.push(format!("item level {}", g(5)));
                }
                if g(7) == "1" {
                    extra.push("new players".to_string());
                }
                if !g(6).is_empty() {
                    extra.push(format!("{}s", g(6)));
                }
                if !extra.is_empty() {
                    s.push_str(&format!(" ({})", extra.join(", ")));
                }
                s
            }
            "r" => match g(1) {
                "t" => "listing timed out".into(),
                "f" => "listing ended, group full".into(),
                _ => "listing ended".into(),
            },
            "s" => format!(
                "searched category {} ({} activities): {} listings, {} shown",
                g(1),
                g(2),
                g(3),
                g(4)
            ),
            "f" => format!("search failed ({})", g(3)),
            "o" => {
                let mut roles = Vec::new();
                for (i, name) in ["tank", "healer", "damage"].iter().enumerate() {
                    if g(i + 1) == "1" {
                        roles.push(*name);
                    }
                }
                format!("finder roles: {}", roles.join("+"))
            }
            "w" => format!("who search: {} shown of {}", g(1), g(2)),
            "d" => {
                let what = match g(1) {
                    "p" => "proposal appeared",
                    "y" => "proposal accepted",
                    "n" => "proposal failed",
                    "r" => "role check",
                    "c" => "ready check",
                    "w" => "completion reward",
                    "o" => "offered to continue",
                    other => other,
                };
                if g(2).is_empty() {
                    format!("dungeon finder: {what}")
                } else {
                    format!("dungeon finder: {what} (dungeon {})", g(2))
                }
            }
            other => format!("{other} {}", f.join(" ")),
        },
        "NS" => {
            let mut bits = Vec::new();
            if g(1) == "-1" {
                bits.push("level ??".to_string());
            } else if !g(1).is_empty() {
                bits.push(format!("level {}", g(1)));
            }
            if !g(2).is_empty() && g(2) != "normal" {
                bits.push(g(2).to_string());
            }
            if !g(3).is_empty() {
                bits.push(g(3).to_lowercase());
            }
            if !g(5).is_empty() {
                bits.push(g(5).to_lowercase());
            }
            if !g(7).is_empty() {
                bits.push(reaction_to_creature(g(7)).to_string());
            }
            if !g(8).is_empty() {
                bits.push(format!("seen at map {} cell {},{}", g(8), g(9), g(10)));
            }
            format!("{}: {}", label('n', g(0), names, "npc"), bits.join(", "))
        }
        "PM" => {
            let mut bits = Vec::new();
            if !g(4).is_empty() {
                bits.push(format!("level {}", g(4)));
            }
            if !g(2).is_empty() {
                bits.push(g(2).to_lowercase());
            }
            if !g(1).is_empty() {
                bits.push(g(1).to_lowercase());
            }
            if !g(3).is_empty() {
                bits.push(g(3).to_string());
            }
            if !g(5).is_empty() {
                bits.push(format!("<{}>", g(5)));
            }
            format!("saw {} ({})", g(0), bits.join(" "))
        }
        "AL" => {
            let mut bits = Vec::new();
            if !g(1).is_empty() {
                bits.push(format!("level {}", g(1)));
            }
            if !g(4).is_empty() {
                bits.push(format!("interaction type {}", g(4)));
            }
            if !g(6).is_empty() {
                bits.push(format!("at {}", g(6)));
            }
            if !g(7).is_empty() {
                bits.push(format!("item {}", g(7)));
            }
            if !g(8).is_empty() {
                bits.push(format!("difficulty {}", g(8)));
            }
            if !g(10).is_empty() {
                bits.push(format!("pinned until {}", g(10)));
            }
            format!("recent ally {} ({})", g(0), bits.join(", "))
        }
        "IV" => {
            let mut s = format!("inspected {}", g(0));
            let mut extra = Vec::new();
            if !g(1).is_empty() {
                extra.push(format!("level {}", g(1)));
            }
            if !g(2).is_empty() {
                extra.push(g(2).to_lowercase());
            }
            if !g(3).is_empty() {
                extra.push(g(3).to_lowercase());
            }
            if !g(4).is_empty() {
                extra.push(g(4).to_string());
            }
            if !g(5).is_empty() {
                extra.push(format!("<{}>", g(5)));
            }
            if !extra.is_empty() {
                s.push_str(&format!(" ({})", extra.join(" ")));
            }
            s
        }
        "CT" => {
            let channel = if g(1).is_empty() {
                String::new()
            } else {
                format!(" {}", g(1))
            };
            format!(
                "{}{channel}: sent {} ({} words), received {} ({} words) over {}s",
                g(0),
                g(2),
                g(3),
                g(4),
                g(5),
                g(6)
            )
        }
        "DQ" => match g(0) {
            "d" => format!("{} asked for a duel to the death", g(1)),
            "f" => "duel finished".into(),
            "o" => "left the duel boundary".into(),
            "i" => "back inside the duel boundary".into(),
            other => format!("{other} {}", f.join(" ")),
        },
        "CE" => match g(0) {
            "p" => format!("{} calendar invites waiting", g(1)),
            "a" => format!("event alarm at {}:{:0>2}", g(1), g(2)),
            "o" => format!(
                "opened calendar event ({})",
                g(1).to_lowercase().replace('_', " ")
            ),
            "n" => {
                if g(1) == "1" {
                    "copied an event".into()
                } else {
                    "started a new event".into()
                }
            }
            "i" => format!("event invite list: {} invited", g(1)),
            "g" => format!("{} guild events", g(1)),
            other => format!("{other} {}", f.join(" ")),
        },
        "CM" => {
            let kind = club_type(g(1));
            let name = if g(2).is_empty() {
                kind.to_string()
            } else {
                format!("{kind} {}", g(2))
            };
            match g(0) {
                "s" => {
                    let mut s = format!("in {name}");
                    if !g(3).is_empty() {
                        s.push_str(&format!(", {} members", g(3)));
                    }
                    if !g(4).is_empty() {
                        s.push_str(&format!(", {}", club_role(g(4))));
                    }
                    s
                }
                "a" => format!("joined {name}"),
                "x" => format!("left {name}"),
                "k" => format!(
                    "removed from a community ({})",
                    match g(3) {
                        "1" => "banned",
                        "2" => "removed",
                        "3" => "community deleted",
                        other => other,
                    }
                ),
                "i" if g(4) == "s" && g(3).is_empty() => format!("still invited to {name}"),
                "i" if g(4) == "s" => format!("still invited to {name} by {}", g(3)),
                "i" if g(3).is_empty() => format!("invited to {name}"),
                "i" => format!("{} invited you to {name}", g(3)),
                "d" => "community invitation removed".into(),
                "r" => format!("{name}: now {}", club_role(g(3))),
                "j" if g(3).is_empty() => format!("a member joined {name}"),
                "j" => format!("{} joined {name}", g(3)),
                "q" => format!("a member left {name}"),
                "f" => format!("applied to a {kind} through the finder"),
                "c" => format!("cancelled an application to a {kind}"),
                "u" => format!("{} finder applications listed", g(3)),
                "v" => format!("{} finder invitations", g(3)),
                "o" => format!("joined {name} while offline"),
                other => format!("{other} {}", f.join(" ")),
            }
        }
        "CV" => {
            let what = if g(1) == "m" {
                format!("movie {}", g(2))
            } else {
                "cinematic".to_string()
            };
            if g(0) == "1" {
                format!("{what} started")
            } else {
                let how = match g(4) {
                    "c" => "skipped",
                    "e" => "failed to play",
                    _ => "watched to the end",
                };
                format!("{what} {how} after {}s", g(3))
            }
        }
        "LL" => format!("loading screen {}s", g(0)),
        "LK" => match g(0) {
            "c" => "logging out".into(),
            "q" => "quitting the game".into(),
            "x" => "logout cancelled".into(),
            other => other.to_string(),
        },
        "UE" => {
            let name = if g(1).is_empty() {
                format!("error {}", g(2))
            } else {
                g(1).to_string()
            };
            let popup = if g(0) == "p" { " (popup)" } else { "" };
            let times = if g(3).is_empty() || g(3) == "1" {
                String::new()
            } else {
                format!(" x{}", g(3))
            };
            format!("{name}{popup}{times}")
        }
        "AO" => match g(0) {
            "s" => format!("{} {} (first list)", g(1), g(2))
                .trim_end()
                .to_string(),
            "a" => format!("added {} {}", g(1), g(2)).trim_end().to_string(),
            "v" => format!("{} {} -> {}", g(1), g(3), g(2)),
            "x" => format!("removed {}", g(1)),
            "c" => format!("client language {}", g(1)),
            "z" => format!("{} SavedVariables too large to save", g(1)),
            other => format!("{other} {}", f.join(" ")),
        },
        "AX" => {
            let what = match g(0) {
                "1" => "boss encounter",
                "2" => "challenge mode",
                "3" => "PvP match",
                "4" => "map",
                "5" => "chat",
                other => other,
            };
            let state = match g(1) {
                "1" => "about to start",
                "2" => "active",
                "0" => "ended",
                other => other,
            };
            format!("addon restriction for {what}: {state}")
        }
        "AF" => {
            let kind = if g(0) == "f" { "forbidden" } else { "blocked" };
            format!("{kind} call {} by {} (x{})", g(2), g(1), g(3))
        }
        "TT" => match g(0) {
            "t" => {
                if g(1) == "1" {
                    format!("support ticket open ({} tickets, status {})", g(2), g(3))
                } else {
                    "no support ticket".into()
                }
            }
            "r" => format!("report ({}): {}", report_type(g(2)), report_result(g(1))),
            "i" => "reported the player who invited you".into(),
            other => format!("{other} {}", f.join(" ")),
        },
        "UW" => {
            let hidden = if g(3) == "0" { " hidden" } else { "" };
            let head = format!("{} {} (set {}){hidden}", g(2), g(1), g(0));
            if g(4).is_empty() {
                head
            } else {
                format!("{head}: {}", g(4))
            }
        }
        "TU" => match g(0) {
            "t" => format!("tutorial hint {}", g(1)),
            "l" => "left the tutorial area".into(),
            other => format!("{other} {}", f.join(" ")),
        },
        "PP" => format!(
            "{} on map {}",
            if g(1) == "1" { "won" } else { "match over" },
            g(0)
        ),
        "RM" => match g(0) {
            "t" => {
                let unit = if g(2).is_empty() { "a unit" } else { g(2) };
                if g(1) == "0" {
                    format!("cleared the marker on {unit}")
                } else {
                    format!("marked {unit} with {}", raid_icon(g(1)))
                }
            }
            "w" => format!("placed world marker {}", g(1)),
            "x" => format!("removed world marker {}", g(1)),
            other => format!("{other} {}", f.join(" ")),
        },
        "RA" => match g(0) {
            "s" => {
                let found = if g(4) == "1" { ", found something" } else { "" };
                format!("survey cast ({} of {} finds){found}", g(2), g(3))
            }
            "f" => format!("find completed ({} of {})", g(2), g(3)),
            "d" => format!("dig site finished (branch {})", g(1)),
            "a" => format!("solved {}", g(4)),
            other => format!("{other} {}", f.join(" ")),
        },
        "SU" => {
            let what = match g(1) {
                "q" => label('q', g(2), names, "quest"),
                "w" => "your waypoint".to_string(),
                "c" => format!("content type {} id {}", g(3), g(2)),
                _ => "nothing".to_string(),
            };
            let start = if g(0) == "s" { " (at start)" } else { "" };
            format!("tracking {what}{start}")
        }
        "BC" => {
            let (kinds, total) = stack_totals(g(1));
            format!("{}: {kinds} items, {total} in all", carried_place(g(0)))
        }
        "RU" => {
            let mut bits = Vec::new();
            if !g(0).is_empty() {
                bits.push(format!("game mode record {}", g(0)));
            }
            if !g(6).is_empty() {
                bits.push(g(6).to_string());
            }
            if !g(8).is_empty() {
                bits.push(format!("server {}", g(8)));
            }
            if !g(9).is_empty() {
                bits.push(format!("realm {}", g(9)));
            }
            if !g(10).is_empty() {
                bits.push(format!("rules {}", g(10)));
            }
            bits.push(format!("self-found {}", yes_no(g(12))));
            bits.push(format!("self-found allowed {}", yes_no(g(13))));
            if g(11) == "c" {
                bits.push("changed".to_string());
            }
            bits.join(", ")
        }
        "QB" => {
            let listed = list_len(g(1));
            format!("{} quests completed ({listed} listed on this line)", g(0))
        }
        "EB" => {
            let maps: Vec<(&str, i64)> = g(0)
                .split(',')
                .filter_map(|m| {
                    let (map, n) = m.split_once(':')?;
                    Some((map, n.parse().ok()?))
                })
                .collect();
            match maps.iter().max_by_key(|(_, n)| *n) {
                Some((map, n)) => format!(
                    "{} zone maps explored, most overlays on map {map} ({n})",
                    maps.len()
                ),
                None => "no zone maps explored".to_string(),
            }
        }
        "KT" => format!("knows {} titles ({} listed)", g(0), list_len(g(1))),
        "IS" => {
            let name = if g(0).is_empty() { "instance" } else { g(0) };
            if g(6) == "2" {
                return format!(
                    "world boss lock {name} (boss {}), resets in {}s",
                    g(1),
                    g(3)
                );
            }
            let mut s = format!(
                "{name} ({}), {}/{} bosses, resets in {}s",
                difficulty_name(g(2)),
                g(8),
                g(9),
                g(3)
            );
            if !g(7).is_empty() {
                s.push_str(&format!(", {} players", g(7)));
            }
            if g(6) == "1" {
                s.push_str(", raid");
            }
            if g(5) == "1" {
                s.push_str(", extended");
            }
            if g(4) == "0" {
                s.push_str(", not locked");
            }
            match g(10) {
                "s" => s.push_str(" (first read)"),
                "c" => s.push_str(" (changed)"),
                _ => {}
            }
            s
        }
        "WX" => format!("weather {} at intensity {}", g(0), g(1)),
        "RQ" => {
            let mut s = format!(
                "{} makes {} from {}",
                spell(0),
                item_count(g(1), g(2), g(3), names),
                if g(4).is_empty() {
                    "no listed reagents".to_string()
                } else {
                    item_stacks(g(4), names)
                }
            );
            if !g(5).is_empty() && g(5) != "0" {
                s.push_str(&format!(", {} optional slots", g(5)));
            }
            if !g(6).is_empty() {
                s.push_str(&format!(", recipe type {}", g(6)));
            }
            s
        }
        "SI" => {
            let cast = if g(2) == "0" {
                "instant".to_string()
            } else {
                format!("{} ms cast", g(2))
            };
            let mut s = format!("{}: {cast}, range {} to {} yards", spell(0), g(3), g(4));
            let costs = power_costs(g(5));
            if !costs.is_empty() {
                s.push_str(&format!(", costs {costs}"));
            }
            if g(6) == "1" {
                s.push_str(", passive");
            }
            s.push_str(&format!(", icon {}", g(1)));
            s
        }
        "SD" => format!("{}: {}", spell(0), g(1)),
        "EL" => format!(
            "{} received {} from encounter {}",
            g(3),
            item_count(g(1), g(2), g(2), names),
            g(0)
        ),
        "AQ" => {
            let what = match g(0) {
                "c" => "commodity search",
                "i" => "item search",
                "b" => "browse row",
                other => other,
            };
            let mut s = format!(
                "{what} {}: lowest {}, {} on sale, {} listings",
                item(1),
                coin(g(2)),
                g(3),
                g(4)
            );
            if !g(5).is_empty() {
                s.push_str(&format!(", item level {}", g(5)));
            }
            if !g(6).is_empty() {
                s.push_str(&format!(", suffix {}", g(6)));
            }
            s
        }
        "HD" => {
            let mut s = if g(0).is_empty() {
                "a hardcore character died".to_string()
            } else {
                format!("{} died", g(0))
            };
            if !g(1).is_empty() {
                s.push_str(&format!(" at level {}", g(1)));
            }
            if !g(2).is_empty() {
                s.push_str(&format!(": {}", g(2)));
            }
            s
        }
        "WH" => {
            let mut s = format!("{} level {}", g(0), g(1));
            let kind = [g(3), g(2)]
                .iter()
                .filter(|v| !v.is_empty())
                .map(|v| v.to_lowercase())
                .collect::<Vec<_>>()
                .join(" ");
            if !kind.is_empty() {
                s.push_str(&format!(" {kind}"));
            }
            if !g(4).is_empty() {
                s.push_str(&format!(" in {}", g(4)));
            }
            if !g(5).is_empty() {
                s.push_str(&format!(" <{}>", g(5)));
            }
            s
        }
        "BO" => {
            let mut s = format!(
                "{} ({}{}): {} kills, {} honorable, {} deaths, {} honor, {} damage, {} healing",
                g(0),
                side(g(1)),
                if g(2).is_empty() {
                    String::new()
                } else {
                    format!(" {}", g(2).to_lowercase())
                },
                g(3),
                g(4),
                g(5),
                g(6),
                g(7),
                g(8)
            );
            if !g(9).is_empty() {
                s.push_str(&format!(", stats {}", g(9)));
            }
            s
        }
        "IG" => {
            let worn: Vec<String> = g(1)
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|entry| {
                    let mut it = entry.split(':');
                    let slot = it.next().unwrap_or("");
                    let id = it.next().unwrap_or("");
                    let enchant = it.next().unwrap_or("");
                    let mut s = format!("slot {slot} {}", label('i', id, names, "item"));
                    if !enchant.is_empty() && enchant != "0" {
                        s.push_str(&format!(" enchant {enchant}"));
                    }
                    s
                })
                .collect();
            format!("{} wears {} items: {}", g(0), worn.len(), worn.join(", "))
        }
        "LG" => {
            if g(0) == "1" {
                "on".into()
            } else {
                "off".into()
            }
        }
        _ => f.join("\t"),
    }
}

/// Readable line: `YYYY-MM-DD HH:MM:SS.cc  <word padded to 9>  <details>`
/// in local time. P lines are path noise, PR is a build probe and SN/IN are
/// name lines; all render to nothing.
pub fn txt_line(ev: &Ev, names: &HashMap<String, String>) -> Option<String> {
    if matches!(ev.code.as_str(), "P" | "SN" | "IN" | "PR") {
        return None;
    }
    let ts = Local
        .timestamp_opt(ev.cs / 100, (ev.cs % 100) as u32 * 10_000_000)
        .single()?;
    let stamp = format!(
        "{}.{:02}",
        ts.format("%Y-%m-%d %H:%M:%S"),
        ev.cs.rem_euclid(100)
    );
    let d = details(&ev.code, &ev.fields, names);
    let mut line = format!("{stamp}  {:<9}", word(&ev.code));
    if !d.is_empty() {
        line.push_str("  ");
        line.push_str(&d);
    }
    Some(line.trim_end().to_string())
}

/// Raw payload line: `<epoch ms>\t<code>\t<fields...>`.
pub fn scl_line(ev: &Ev) -> String {
    let mut s = format!("{}\t{}", ev.cs * 10, ev.code);
    if !ev.fields.is_empty() {
        s.push('\t');
        s.push_str(&ev.fields.join("\t"));
    }
    s
}

/// A character's name as the app shows it: first name and surname, `Name-Surname` as `Name Surname`.
/// Recordings, payloads and `scarlet.txt` keep the stored `Name-Surname`.
pub fn display_name(name: &str) -> String {
    name.replacen('-', " ", 1)
}

fn player_shaped(name: &str) -> bool {
    let part = |p: &str| {
        let mut c = p.chars();
        c.next().is_some_and(char::is_uppercase) && c.all(|c| c.is_lowercase() || c == '\'')
    };
    name.split_once('-')
        .is_some_and(|(a, b)| part(a) && part(b))
}

fn player_fields(code: &str, f: &[String]) -> &'static [usize] {
    let kind = f.first().map_or("", String::as_str);
    match (code, kind) {
        ("DR", _) if f.get(2).is_some_and(|s| player_shaped(s)) => &[2],
        ("GL", "c") => &[1, 2],
        ("GL", "m" | "l" | "p") | ("RC", "s" | "r") => &[1],
        ("CM", "i" | "j") => &[3],
        ("GI", "e") => &[],
        ("PK" | "LW" | "IV" | "PM" | "AL" | "DU" | "HD" | "WH" | "BO" | "IG", _) => &[0],
        ("EL", _) => &[3],
        ("GM" | "DP" | "QA" | "ML" | "TR" | "LT" | "LD" | "FR" | "GI" | "DW" | "DQ", _) => &[1],
        ("LM", _) => &[2],
        ("GU", _) => &[5],
        _ => &[],
    }
}

/// `(word, details)` for an event, the two halves of a rendered timeline
/// line without the timestamp, with player names as `display_name` shows them.
/// Unknown codes render as the code plus the fields joined, never an error.
pub fn event_parts(ev: &Ev, names: &HashMap<String, String>) -> (String, String) {
    let at = player_fields(&ev.code, &ev.fields);
    let text = if at.is_empty() {
        details(&ev.code, &ev.fields, names)
    } else {
        let mut fields = ev.fields.clone();
        for &i in at {
            if let Some(v) = fields.get_mut(i) {
                *v = display_name(v);
            }
        }
        details(&ev.code, &fields, names)
    };
    (word(&ev.code).to_string(), text)
}

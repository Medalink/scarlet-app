//! The archive of imported history (Scarlet's FORMAT.md, "Imported history"): every record read from
//! another addon's saved variables, kept per character in `imports/archive.tsv` as the addon's own
//! values, so a record the addon later drops stays and a corrected mapping reaches it. Other parties'
//! names are never archived.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub(crate) const DIR: &str = "imports";
const MAGIC: &str = "scarletimportsarchive";

/// One record as an addon wrote it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    /// TradeSkillMaster's gold log: the minute and the balance in copper.
    TsmGold { minute: i64, copper: i64 },
    /// A TradeSkillMaster sale or purchase: the unit price and the quantity, as TSM stores them.
    TsmTrade {
        sale: bool,
        item: i64,
        stack: i64,
        qty: i64,
        price: i64,
        time: i64,
        source: String,
    },
    /// A TradeSkillMaster income or expense: its type text and the amount.
    TsmMoney {
        income: bool,
        kind: String,
        amount: i64,
        time: i64,
    },
    /// A BeanCounter completed auction or bid, with the money as BeanCounter wrote it.
    BeanTrade {
        sale: bool,
        item: i64,
        stack: i64,
        money: i64,
        time: i64,
    },
    /// Mastercraft's last balance of one local day, `YYYY-MM-DD`.
    McDay { date: String, copper: i64 },
    /// An auction TradeSkillMaster took back from the mail, expired or cancelled: its item string,
    /// stack size, the items returned and the row's time.
    TsmReturn {
        cancelled: bool,
        item: String,
        stack: i64,
        qty: i64,
        time: i64,
    },
}

fn flag(on: bool, yes: &'static str, no: &'static str) -> &'static str {
    if on { yes } else { no }
}

impl Entry {
    fn fields(&self) -> Vec<String> {
        match self {
            Entry::TsmGold { minute, copper } => {
                vec!["tsm-gold".into(), minute.to_string(), copper.to_string()]
            }
            Entry::TsmTrade {
                sale,
                item,
                stack,
                qty,
                price,
                time,
                source,
            } => vec![
                flag(*sale, "tsm-sale", "tsm-buy").into(),
                item.to_string(),
                stack.to_string(),
                qty.to_string(),
                price.to_string(),
                time.to_string(),
                source.clone(),
            ],
            Entry::TsmMoney {
                income,
                kind,
                amount,
                time,
            } => vec![
                flag(*income, "tsm-income", "tsm-expense").into(),
                kind.clone(),
                amount.to_string(),
                time.to_string(),
            ],
            Entry::BeanTrade {
                sale,
                item,
                stack,
                money,
                time,
            } => vec![
                flag(*sale, "bean-sale", "bean-buy").into(),
                item.to_string(),
                stack.to_string(),
                money.to_string(),
                time.to_string(),
            ],
            Entry::McDay { date, copper } => {
                vec!["mc-day".into(), date.clone(), copper.to_string()]
            }
            Entry::TsmReturn {
                cancelled,
                item,
                stack,
                qty,
                time,
            } => vec![
                flag(*cancelled, "tsm-cancelled", "tsm-expired").into(),
                item.clone(),
                stack.to_string(),
                time.to_string(),
                qty.to_string(),
            ],
        }
    }

    fn key(&self) -> String {
        let f = self.fields();
        let n = match self {
            Entry::TsmGold { .. } | Entry::McDay { .. } => 2,
            Entry::TsmReturn { .. } => 4,
            _ => f.len(),
        };
        f[..n].join("\t")
    }

    fn line(&self) -> String {
        self.fields().join("\t")
    }

    fn parse(line: &str) -> Option<Entry> {
        let f: Vec<&str> = line.split('\t').collect();
        let int = |i: usize| f.get(i)?.parse::<i64>().ok();
        let entry = match *f.first()? {
            "tsm-gold" if f.len() == 3 => Entry::TsmGold {
                minute: int(1)?,
                copper: int(2)?,
            },
            tag @ ("tsm-sale" | "tsm-buy") if f.len() == 7 => Entry::TsmTrade {
                sale: tag == "tsm-sale",
                item: int(1)?,
                stack: int(2)?,
                qty: int(3)?,
                price: int(4)?,
                time: int(5)?,
                source: f[6].to_string(),
            },
            tag @ ("tsm-income" | "tsm-expense") if f.len() == 4 => Entry::TsmMoney {
                income: tag == "tsm-income",
                kind: f[1].to_string(),
                amount: int(2)?,
                time: int(3)?,
            },
            tag @ ("bean-sale" | "bean-buy") if f.len() == 5 => Entry::BeanTrade {
                sale: tag == "bean-sale",
                item: int(1)?,
                stack: int(2)?,
                money: int(3)?,
                time: int(4)?,
            },
            "mc-day" if f.len() == 3 => Entry::McDay {
                date: f[1].to_string(),
                copper: int(2)?,
            },
            tag @ ("tsm-expired" | "tsm-cancelled") if f.len() == 5 => Entry::TsmReturn {
                cancelled: tag == "tsm-cancelled",
                item: f[1].to_string(),
                stack: int(2)?,
                time: int(3)?,
                qty: int(4)?,
            },
            _ => return None,
        };
        Some(entry)
    }
}

/// Every record a character's addons have ever shown, one per key.
#[derive(Default)]
pub struct Archive {
    entries: BTreeMap<String, Entry>,
}

fn archive_path(char_dir: &Path) -> PathBuf {
    char_dir.join(DIR).join("archive.tsv")
}

impl Archive {
    /// A character's archive, empty when it has none yet. A line that does not read is an error, so
    /// nothing is written over records this reader does not understand.
    pub fn load(char_dir: &Path) -> Result<Archive, String> {
        let path = archive_path(char_dir);
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Archive::default()),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        let mut lines = text.lines();
        let head: Vec<&str> = lines.next().unwrap_or("").split('\t').collect();
        if head.first() != Some(&MAGIC) || head.get(1) != Some(&"1") {
            return Err(format!(
                "{}: line 1 is not an archive header",
                path.display()
            ));
        }
        let mut entries = BTreeMap::new();
        for (i, line) in lines.enumerate() {
            let Some(entry) = Entry::parse(line) else {
                return Err(format!("{}: line {} does not read", path.display(), i + 2));
            };
            entries.insert(entry.key(), entry);
        }
        Ok(Archive { entries })
    }

    /// Adds what was read. A key read again with other values replaces its record; nothing is
    /// removed. Returns how many records were added or replaced.
    pub fn merge(&mut self, read: Vec<Entry>) -> usize {
        let mut changed = 0;
        for entry in read {
            let key = entry.key();
            if self.entries.get(&key) == Some(&entry) {
                continue;
            }
            self.entries.insert(key, entry);
            changed += 1;
        }
        changed
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.entries.values()
    }

    fn text(&self) -> String {
        let mut out = format!("{MAGIC}\t1\n");
        for entry in self.entries.values() {
            out.push_str(&entry.line());
            out.push('\n');
        }
        out
    }

    /// Writes the archive through a temp file, only when its text differs from the file's.
    pub fn save(&self, char_dir: &Path) -> io::Result<bool> {
        let path = archive_path(char_dir);
        let bytes = self.text().into_bytes();
        if fs::read(&path).is_ok_and(|old| old == bytes) {
            return Ok(false);
        }
        fs::create_dir_all(char_dir.join(DIR))?;
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, bytes)?;
        fs::rename(&tmp, &path)?;
        Ok(true)
    }
}

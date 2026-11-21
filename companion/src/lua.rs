//! Hand-written parser for the Lua table literal subset WoW writes to
//! SavedVariables: tables with `["key"] = value` pairs and bare array items,
//! double-quoted strings with escapes (including `\ddd` decimal byte
//! escapes), integers, floats, booleans and nil. No Lua runtime.

// The parser understands the whole literal subset; not every variant is
// read back (floats, bools appear only in ignored tables like `opt`).
#[allow(dead_code)]
#[derive(Debug)]
pub enum Lua {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Nil,
    Table(Vec<(String, Lua)>),
}

impl Lua {
    pub fn get(&self, key: &str) -> Option<&Lua> {
        match self {
            Lua::Table(items) => items.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Lua::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Lua::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// Any number, integer or float.
    pub fn as_num(&self) -> Option<f64> {
        match self {
            Lua::Int(i) => Some(*i as f64),
            Lua::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// Array-style items of a table, in written order.
    pub fn items(&self) -> Option<&[(String, Lua)]> {
        match self {
            Lua::Table(items) => Some(items),
            _ => None,
        }
    }
}

/// Parse a SavedVariables file body down to the value assigned to its first
/// global (conventionally `ScarletDB`).
pub fn parse_db(text: &str) -> Result<Lua, String> {
    let mut p = P {
        b: text.as_bytes(),
        i: 0,
    };
    p.skip_ws();
    while p.peek().is_some_and(|c| c != b'=') {
        p.i += 1;
    }
    p.expect(b'=')?;
    p.value()
}

struct P<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> P<'a> {
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    /// Skip whitespace and `--` line comments (including the `-- [n]`
    /// markers WoW appends after array items).
    fn skip_ws(&mut self) {
        loop {
            while self
                .peek()
                .is_some_and(|c| matches!(c, b' ' | b'\t' | b'\r' | b'\n'))
            {
                self.i += 1;
            }
            if self.peek() == Some(b'-') && self.b.get(self.i + 1) == Some(&b'-') {
                while self.peek().is_some_and(|c| c != b'\n') {
                    self.i += 1;
                }
            } else {
                return;
            }
        }
    }

    fn eat(&mut self, c: u8) -> bool {
        self.skip_ws();
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, c: u8) -> Result<(), String> {
        if self.eat(c) {
            Ok(())
        } else {
            Err(format!("expected '{}' at byte {}", c as char, self.i))
        }
    }

    fn kw(&mut self, kw: &str) -> bool {
        if self.b[self.i..].starts_with(kw.as_bytes()) {
            self.i += kw.len();
            true
        } else {
            false
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.skip_ws();
        if self.peek() != Some(b'"') {
            return Err(format!("expected string at byte {}", self.i));
        }
        self.i += 1;
        let mut out: Vec<u8> = Vec::new();
        loop {
            match self.peek() {
                None => return Err("unterminated string".into()),
                Some(b'"') => {
                    self.i += 1;
                    return String::from_utf8(out).map_err(|_| "invalid utf-8 in string".into());
                }
                Some(b'\\') => {
                    self.i += 1;
                    let Some(c) = self.peek() else {
                        return Err("unterminated escape".into());
                    };
                    self.i += 1;
                    match c {
                        b'a' => out.push(0x07),
                        b'b' => out.push(0x08),
                        b'f' => out.push(0x0c),
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'v' => out.push(0x0b),
                        b'\\' | b'"' | b'\'' => out.push(c),
                        b'0'..=b'9' => {
                            let mut v = u32::from(c - b'0');
                            for _ in 0..2 {
                                if let Some(d @ b'0'..=b'9') = self.peek() {
                                    v = v * 10 + u32::from(d - b'0');
                                    self.i += 1;
                                }
                            }
                            out.push(v.min(255) as u8);
                        }
                        _ => out.push(c),
                    }
                }
                Some(_) => {
                    let text = std::str::from_utf8(&self.b[self.i..])
                        .map_err(|_| "invalid utf-8".to_string())?;
                    let ch = text.chars().next().unwrap();
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                    self.i += ch.len_utf8();
                }
            }
        }
    }

    fn number(&mut self) -> Result<Lua, String> {
        let start = self.i;
        if matches!(self.peek(), Some(b'-' | b'+')) {
            self.i += 1;
        }
        let mut float = false;
        while let Some(c) = self.peek() {
            match c {
                b'0'..=b'9' => self.i += 1,
                b'.' | b'e' | b'E' => {
                    float = true;
                    self.i += 1;
                }
                _ => break,
            }
        }
        let s =
            std::str::from_utf8(&self.b[start..self.i]).map_err(|_| "bad number".to_string())?;
        if float {
            s.parse::<f64>()
                .map(Lua::Float)
                .map_err(|_| format!("bad number {s}"))
        } else {
            s.parse::<i64>()
                .map(Lua::Int)
                .map_err(|_| format!("bad number {s}"))
        }
    }

    fn table(&mut self) -> Result<Lua, String> {
        self.expect(b'{')?;
        let mut items: Vec<(String, Lua)> = Vec::new();
        let mut idx: u64 = 1;
        loop {
            self.skip_ws();
            match self.peek() {
                None => return Err("unterminated table".into()),
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Lua::Table(items));
                }
                _ => {}
            }
            let key = if self.peek() == Some(b'[') {
                self.i += 1;
                let k = self.value()?;
                self.expect(b']')?;
                self.expect(b'=')?;
                match k {
                    Lua::Str(s) => s,
                    other => format!("{other:?}"),
                }
            } else {
                let k = idx.to_string();
                idx += 1;
                k
            };
            let v = self.value()?;
            items.push((key, v));
            if !self.eat(b',') && !self.eat(b';') {
                self.skip_ws();
                if self.peek() == Some(b'}') {
                    self.i += 1;
                    return Ok(Lua::Table(items));
                }
                return Err(format!("expected ',' at byte {}", self.i));
            }
        }
    }

    fn value(&mut self) -> Result<Lua, String> {
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.table(),
            Some(b'"') => self.string().map(Lua::Str),
            Some(b't') if self.kw("true") => Ok(Lua::Bool(true)),
            Some(b'f') if self.kw("false") => Ok(Lua::Bool(false)),
            Some(b'n') if self.kw("nil") => Ok(Lua::Nil),
            Some(c) if c.is_ascii_digit() || c == b'-' || c == b'+' => self.number(),
            _ => Err(format!("unexpected value at byte {}", self.i)),
        }
    }
}

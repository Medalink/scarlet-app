//! The session recap over the app's store. The builder is the companion's (`scarlet::recap`), so the app and
//! `Recaps.lua` give the same numbers; this reads the store's events for it.

use crate::store::{Row, Store};
use scarlet::recap::Tally;
use scarlet::recap::{Events, Facts, Rec};
pub use scarlet::recap::{SessionRecap, Title};
use std::iter::Peekable;
use std::sync::Arc;

struct Source<'a> {
    st: &'a Store,
    ch: u32,
    paths: bool,
}

type Recs<'a> = Box<dyn Iterator<Item = Rec<'a>> + 'a>;

fn recs_of(rows: &[Row], ch: u32, from: i64, to: i64) -> Recs<'_> {
    let start = rows.partition_point(|r| r.ms < from);
    Box::new(
        rows[start..]
            .iter()
            .take_while(move |r| r.ms <= to)
            .filter(move |r| r.ch == ch)
            .map(|r| Rec {
                ms: r.ms,
                code: r.code.as_str(),
                fields: r.fields.as_slice(),
            }),
    )
}

struct Interleave<'a> {
    events: Peekable<Recs<'a>>,
    paths: Peekable<Recs<'a>>,
}

impl<'a> Iterator for Interleave<'a> {
    type Item = Rec<'a>;

    fn next(&mut self) -> Option<Rec<'a>> {
        match (self.events.peek(), self.paths.peek()) {
            (Some(e), Some(p)) if p.ms < e.ms => self.paths.next(),
            (None, Some(_)) => self.paths.next(),
            _ => self.events.next(),
        }
    }
}

impl Events for Source<'_> {
    fn between(&self, from: i64, to: i64) -> Recs<'_> {
        let events = recs_of(&self.st.events, self.ch, from, to);
        if !self.paths {
            return events;
        }
        Box::new(Interleave {
            events: events.peekable(),
            paths: recs_of(&self.st.paths, self.ch, from, to).peekable(),
        })
    }
}

fn span(st: &Store, session: usize) -> (i64, i64) {
    (st.sessions[session].start, st.sessions[session].end)
}

fn with_facts<T>(st: &Store, ch: u32, all: bool, paths: bool, f: impl FnOnce(&Facts) -> T) -> T {
    let sessions: Vec<(i64, i64)> = if all {
        st.sessions
            .iter()
            .filter(|s| s.ch == ch)
            .map(|s| (s.start, s.end))
            .collect()
    } else {
        Vec::new()
    };
    let character = st.chars.get(ch as usize).map_or("", String::as_str);
    let source = Source { st, ch, paths };
    f(&Facts {
        character,
        events: &source,
        names: &st.names,
        sessions: &sessions,
    })
}

/// The recap of one character over the store sessions `sessions` (indices into `st.sessions`), merged into one.
pub fn build(st: &Store, sessions: &[usize]) -> Option<SessionRecap> {
    let ch = st.sessions[*sessions.first()?].ch;
    let spans: Vec<(i64, i64)> = sessions.iter().map(|i| span(st, *i)).collect();
    let mut recap = with_facts(st, ch, true, true, |f| scarlet::recap::build(f, &spans))?;
    scarlet::featscore::fill(&mut recap, &tallies(st, ch));
    Some(recap)
}

/// The recap of character `ch` over `spans` (ms), merged into one; the time played counts an overlap once.
pub fn build_spans(st: &Store, ch: u32, spans: &[(i64, i64)]) -> Option<SessionRecap> {
    let mut recap = with_facts(st, ch, true, true, |f| scarlet::recap::build(f, spans))?;
    scarlet::featscore::fill(&mut recap, &tallies(st, ch));
    Some(recap)
}

/// The recap a card of the sessions list draws, of character `ch` over `spans` (ms).
pub fn lite_spans(st: &Store, ch: u32, spans: &[(i64, i64)]) -> Option<SessionRecap> {
    with_facts(st, ch, false, false, |f| scarlet::recap::lite(f, spans))
}

/// The recap a card of the sessions list draws.
pub fn lite(st: &Store, session: usize) -> Option<SessionRecap> {
    lite_spans(st, st.sessions[session].ch, &[span(st, session)])
}

pub fn title_of(st: &Store, session: usize) -> Title {
    with_facts(st, st.sessions[session].ch, false, false, |f| {
        scarlet::recap::title_of(f, span(st, session))
    })
}

/// The sessions of one character as tallies, for counting its feats.
pub fn tallies(st: &Store, ch: u32) -> Arc<Vec<Tally>> {
    if let Some(done) = st.tallies().get(&ch) {
        return done.clone();
    }
    let made = Arc::new(with_facts(st, ch, true, false, scarlet::recap::tallies));
    st.tallies().insert(ch, made.clone());
    made
}

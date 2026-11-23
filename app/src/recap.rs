//! The session recap over the app's store. The builder is the companion's (`scarlet::recap`), so the app and
//! `Recaps.lua` give the same numbers; this reads the store's events for it.

use crate::store::Store;
use scarlet::recap::{Events, Facts, Rec};
pub use scarlet::recap::{SessionRecap, Title};

struct Source<'a> {
    st: &'a Store,
    ch: u32,
}

impl Events for Source<'_> {
    fn between(&self, from: i64, to: i64) -> Box<dyn Iterator<Item = Rec<'_>> + '_> {
        let start = self.st.events.partition_point(|r| r.ms < from);
        Box::new(
            self.st.events[start..]
                .iter()
                .take_while(move |r| r.ms <= to)
                .filter(|r| r.ch == self.ch)
                .map(|r| Rec {
                    ms: r.ms,
                    code: r.code.as_str(),
                    fields: r.fields.as_slice(),
                }),
        )
    }
}

fn span(st: &Store, session: usize) -> (i64, i64) {
    (st.sessions[session].start, st.sessions[session].end)
}

fn with_facts<T>(st: &Store, ch: u32, all: bool, f: impl FnOnce(&Facts) -> T) -> T {
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
    let source = Source { st, ch };
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
    with_facts(st, ch, true, |f| scarlet::recap::build(f, &spans))
}

/// The recap a card of the sessions list draws.
pub fn lite(st: &Store, session: usize) -> Option<SessionRecap> {
    with_facts(st, st.sessions[session].ch, false, |f| {
        scarlet::recap::lite(f, span(st, session))
    })
}

pub fn title_of(st: &Store, session: usize) -> Title {
    with_facts(st, st.sessions[session].ch, false, |f| {
        scarlet::recap::title_of(f, span(st, session))
    })
}

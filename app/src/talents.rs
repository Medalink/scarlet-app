//! The Talents view: one character's build by tab, the order points were taken in, and respecs.

use crate::commands::with_store;
use scarlet::talents::{self, CODES, Line, Talents};
use tauri::AppHandle;

#[tauri::command(async)]
pub fn talents(app: AppHandle, ch: Option<String>) -> Option<Talents> {
    with_store(&app, |st| {
        let character = ch.and_then(|n| st.char_index(&n))?;
        Some(talents::build(
            st.events
                .iter()
                .filter(|r| r.ch == character && CODES.contains(&r.code.as_str()))
                .map(|r| Line {
                    ms: r.ms,
                    code: &r.code,
                    fields: &r.fields,
                }),
        ))
    })
}

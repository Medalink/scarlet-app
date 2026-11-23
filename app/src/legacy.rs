//! The Legacy view: one character's Legacy talents, their ranks over time and what each rank did.

use crate::commands::with_store;
use scarlet::legacy::{self, CODES, Legacy};
use scarlet::talents::Line;
use tauri::AppHandle;

#[tauri::command(async)]
pub fn legacy(app: AppHandle, ch: Option<String>) -> Option<Legacy> {
    with_store(&app, |st| {
        let character = ch.and_then(|n| st.char_index(&n))?;
        Some(legacy::build(
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

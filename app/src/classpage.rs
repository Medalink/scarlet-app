//! The class view: the website's class page of a character, read over the install's signed
//! request. With uploading off, or before this install has registered with the website, nothing
//! here makes a network request.

use crate::commands::blocking;
use crate::feats::registered_session;
use crate::settings;
use crate::updater;
use crate::upload::open_url;
use base64::Engine;
use scarlet::upload::config;
use serde::Serialize;
use serde_json::Value;
use std::io::Read;

/// What the class view draws from.
#[derive(Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ClassPageState {
    /// Uploading is off, or this install has not registered with the website.
    Off,
    /// The website has no class page for the character.
    None,
    /// The website's page of the character, as it answered.
    Page {
        page: Value,
    },
    Error {
        message: String,
    },
}

#[tauri::command]
pub async fn class_page_get(character: String) -> ClassPageState {
    blocking(move || {
        let mut sess = match registered_session() {
            Ok(Some(sess)) => sess,
            Ok(None) => return ClassPageState::Off,
            Err(message) => return ClassPageState::Error { message },
        };
        match sess.class_page(&character) {
            Ok(v) => match v
                .get("characters")
                .and_then(Value::as_array)
                .and_then(|l| l.first())
            {
                Some(page) => ClassPageState::Page { page: page.clone() },
                None => ClassPageState::None,
            },
            Err(e) => ClassPageState::Error {
                message: e.to_string(),
            },
        }
    })
    .await
}

fn site() -> String {
    config::site(&settings::load().upload.server)
}

fn path_without_host(url: &str) -> Option<&str> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    rest.find('/').map(|at| &rest[at..])
}

const ICON_FOLDERS: [&str; 2] = ["/spell-icons/", "/item-icons/"];
const MAX_ICON: u64 = 256 * 1024;

/// A game icon the class page names, as a `data:` address: the page may not load images from the
/// website itself, so the app fetches the one folder of icons the website serves and hands the
/// bytes over.
#[tauri::command]
pub async fn class_icon(url: String) -> Result<String, String> {
    blocking(move || {
        let path = path_without_host(&url)
            .filter(|p| ICON_FOLDERS.iter().any(|f| p.starts_with(f)) && !p.contains(".."))
            .ok_or("That is not an icon address.")?;
        let mut resp = updater::http_agent()
            .get(&format!("{}{path}", site()))
            .call()
            .map_err(|e| e.to_string())?;
        let mut bytes = Vec::new();
        resp.body_mut()
            .with_config()
            .limit(MAX_ICON)
            .reader()
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        Ok(format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ))
    })
    .await
}

/// Opens a character's class page on the website in the browser.
#[tauri::command]
pub async fn class_page_open(url: String) -> Result<(), String> {
    blocking(move || {
        let path = path_without_host(&url)
            .filter(|p| p.starts_with("/c/"))
            .ok_or("That is not a class page address.")?;
        open_url(&format!("{}{path}", site()))
    })
    .await
}

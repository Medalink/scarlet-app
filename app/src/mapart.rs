//! Where the Map view's art comes from: Scarlet's own dark sheets, or the player's OpenMap addon.

use crate::commands::state;
use crate::openmap::{self, Install};
use crate::settings;
use serde::Serialize;
use tauri::{AppHandle, Manager};

/// OpenMap's page, the one address `open_openmap_page` opens.
const OPENMAP_URL: &str = "https://github.com/openmap-world/openmap";

#[derive(Serialize)]
pub struct ArtStatus {
    /// "dark" or "openmap": the setting.
    art: String,
    /// OpenMap's files were found in the WoW folder from Settings.
    installed: bool,
    /// When not: "no-wow" (no WoW folder), "no-forever" (no `_forever_` in it), "no-addon" (no
    /// OpenMap in its AddOns), "broken-addon" (OpenMap there without its map files).
    reason: Option<&'static str>,
    version: Option<String>,
    wow: String,
}

#[tauri::command(async)]
pub fn map_art_status(app: AppHandle) -> ArtStatus {
    let s = state(&app).settings.lock().unwrap().clone();
    let found = Install::probe(&s.wow);
    ArtStatus {
        art: s.map_art,
        installed: found.is_ok(),
        reason: found.as_ref().err().copied(),
        version: found.ok().map(|i| i.version),
        wow: s.wow.display().to_string(),
    }
}

#[tauri::command(async)]
pub fn set_map_art(app: AppHandle, art: String) -> Result<String, String> {
    if art != "dark" && art != "openmap" {
        return Err("Map art is dark or openmap.".into());
    }
    settings::update(|c| c.map_art = art.clone())?;
    state(&app).settings.lock().unwrap().map_art = art.clone();
    Ok(art)
}

/// One map's picture cut out of the OpenMap install, as WebP; empty when OpenMap is not installed or
/// does not have the map. With `land` the art outside the zone's own land is faded (the source of a
/// dark sheet); without it the art is as OpenMap draws it. The first cut of a map takes about a tenth of a second and is cached.
#[tauri::command]
pub async fn map_image_openmap(
    app: AppHandle,
    map: u32,
    land: Option<bool>,
) -> tauri::ipc::Response {
    let (wow, data) = {
        let s = state(&app).settings.lock().unwrap();
        (s.wow.clone(), s.data.clone())
    };
    // Finding the install reads files too, so it runs on the blocking thread with the cut.
    let bytes = tauri::async_runtime::spawn_blocking(move || {
        Install::find(&wow).and_then(|i| openmap::image(&i, &data, map, land.unwrap_or(false)))
    })
    .await
    .ok()
    .flatten();
    tauri::ipc::Response::new(bytes.unwrap_or_default())
}

/// A map's pre-rendered dark sheet from the app's `dark` resource folder; empty when the folder has
/// none, and the view then draws the sheet itself from whichever art it has.
#[tauri::command(async)]
pub fn map_image_dark(app: AppHandle, map: u32) -> tauri::ipc::Response {
    let bytes = app
        .path()
        .resource_dir()
        .ok()
        .map(|d| d.join("dark").join(format!("{map}.webp")))
        .and_then(|p| std::fs::read(p).ok());
    tauri::ipc::Response::new(bytes.unwrap_or_default())
}

/// Opens OpenMap's page in the default browser. Takes no address, so the page cannot open anything else.
#[tauri::command(async)]
pub fn open_openmap_page() -> Result<(), String> {
    use windows_sys::Win32::System::Com::{
        COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
    };
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let wide = |s: &str| {
        s.encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<u16>>()
    };
    // ShellExecute wants COM on the calling thread, and a command that runs off the window thread
    // has none. A success code (S_OK, or S_FALSE when the thread already had it) is paired with an
    // uninitialize; a thread that already holds COM in another mode needs neither.
    // SAFETY: the reserved argument must be null; the thread is the one that calls Uninitialize.
    let com = unsafe {
        CoInitializeEx(
            std::ptr::null(),
            (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
        )
    } >= 0;
    // SAFETY: both strings are NUL-terminated and outlive the call; the other arguments are null.
    let rc = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            wide("open").as_ptr(),
            wide(OPENMAP_URL).as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    } as usize;
    if com {
        // SAFETY: pairs the successful CoInitializeEx above, on the same thread.
        unsafe { CoUninitialize() };
    }
    // ShellExecute reports success with a value above 32.
    if rc > 32 {
        Ok(())
    } else {
        Err(format!("Windows could not open the page (code {rc})."))
    }
}

/// `scarlet-app --openmap-cut <folder> [--plain]`: cuts every map out of the OpenMap install in Settings
/// into the folder, with no window, and writes `cut.log` there (the build has no console). The pictures
/// are the sources of the dark sheets, with the art outside each zone's land faded; `--plain` keeps the
/// art as OpenMap draws it (the Game map look). For measuring a full cache and for tools that want the
/// pictures as files.
pub fn cut_to(dir: &std::path::Path, land: bool) {
    let log = dir.join("cut.log");
    let _ = std::fs::create_dir_all(dir);
    let s = settings::load();
    let Some(inst) = Install::find(&s.wow) else {
        let _ = std::fs::write(log, format!("no OpenMap in {}\n", s.wow.display()));
        return;
    };
    let t = std::time::Instant::now();
    let (mut maps, mut bytes) = (0u32, 0usize);
    let r = openmap::build_all(&inst, dir, land, |_, n| {
        maps += 1;
        bytes += n;
    });
    let note = match r {
        Ok(()) => format!(
            "OpenMap {} cut {maps} maps, {bytes} bytes, {:.2} s\n",
            inst.version,
            t.elapsed().as_secs_f64()
        ),
        Err(e) => format!("cut failed after {maps} maps: {e}\n"),
    };
    let _ = std::fs::write(log, note);
}

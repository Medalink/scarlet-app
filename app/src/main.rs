//! Scarlet: the desktop viewer. Loads every payload under the data folder
//! into memory and serves aggregates to the static UI in ui/.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agg;
mod commands;
mod launch;
mod loot;
mod ops;
mod settings;
mod store;

fn main() {
    // The background copy that opens Scarlet when WoW Forever starts; no window, no Tauri.
    if std::env::args().any(|a| a == launch::FLAG) {
        launch::watch();
        return;
    }
    // One Scarlet window: a second launch brings the open one to the front and exits.
    if !launch::claim_app() {
        launch::focus_existing();
        return;
    }

    // Temp on E:, never C:. Safe here: before any thread spawns.
    let temp = settings::temp_dir();
    unsafe {
        std::env::set_var("TMP", &temp);
        std::env::set_var("TEMP", &temp);
    }

    let run = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::app_state,
            commands::overview,
            commands::stats,
            commands::heatmap,
            commands::timeline,
            commands::sessions,
            commands::map_data,
            commands::map_image,
            commands::charts,
            commands::trends,
            loot::loot,
            ops::compress_export,
            ops::compress_logs,
            ops::toggle_watch,
            ops::sizes,
            ops::get_settings,
            ops::set_settings,
            ops::save_view,
            ops::set_open_with_wow,
        ])
        .setup(move |app| {
            let dir = settings::webview_data();
            std::fs::create_dir_all(&dir).ok();
            tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("Scarlet")
            .inner_size(1280.0, 840.0)
            .min_inner_size(960.0, 620.0)
            .data_directory(dir)
            .build()?;

            let s = settings::load();
            if s.watch {
                ops::set_watch(app.handle(), true);
            }
            // Keeps the startup entry pointing at this exe and the watcher running after
            // a rebuild or a kill.
            if s.open_with_wow {
                let _ = launch::set_registered(true);
                launch::spawn_watcher();
            }
            Ok(())
        })
        .run(tauri::generate_context!());
    // No console in release builds: a startup failure goes to scarlet.log beside the exe.
    if let Err(e) = run {
        let _ = std::fs::write(
            settings::exe_dir().join("scarlet.log"),
            format!("scarlet failed to start: {e}\n"),
        );
    }
}

//! Scarlet: the desktop viewer. Loads every payload under the data folder
//! into memory and serves aggregates to the static UI in ui/.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod addon_sync;
mod addons;
mod agg;
mod browse;
mod classpage;
mod commands;
mod discover;
mod feats;
mod feats_catalog;
mod items;
mod launch;
mod layers;
mod legacy;
mod lifetime;
mod loot;
mod mapart;
mod openmap;
mod ops;
mod recap;
mod roster;
mod run;
mod session_groups;
mod settings;
mod store;
mod talents;
mod updater;
mod upload;
mod watch;

/// `--home <folder>` (or `--home=<folder>`) as `SCARLET_HOME`: the installer restarts the app after an
/// update with the arguments it had, but not its environment. Runs before any thread starts.
fn home_from_args() {
    if std::env::var_os("SCARLET_HOME").is_some_and(|v| !v.is_empty()) {
        return;
    }
    let mut args = std::env::args_os().skip(1);
    while let Some(a) = args.next() {
        let home = if a == "--home" {
            args.next()
        } else {
            a.to_str()
                .and_then(|s| s.strip_prefix("--home="))
                .map(std::ffi::OsString::from)
        };
        if let Some(h) = home.filter(|h| !h.is_empty()) {
            // SAFETY: first thing main does, before any thread exists.
            unsafe { std::env::set_var("SCARLET_HOME", h) };
            return;
        }
    }
}

fn main() {
    home_from_args();
    // The background copy that opens Scarlet when WoW Forever starts; no window, no Tauri.
    if std::env::args().any(|a| a == launch::FLAG) {
        launch::watch();
        return;
    }
    // A tool run: cuts the OpenMap maps into a folder and exits.
    let mut args = std::env::args();
    if args.any(|a| a == "--openmap-cut") {
        if let Some(dir) = args.next() {
            let plain = args.next().is_some_and(|a| a == "--plain");
            mapart::cut_to(std::path::Path::new(&dir), !plain);
        }
        return;
    }
    updater::note_start();
    if let Some(run) = run::from_args() {
        run::set_pending(run);
    }
    // One Scarlet window: a second launch brings the open one to the front and exits.
    let restarted = std::env::args().any(|a| a == launch::RESTARTED);
    let mut tries = 0;
    while !launch::claim_app() {
        if !restarted || tries >= launch::RESTART_WAIT_TRIES {
            launch::focus_existing();
            return;
        }
        tries += 1;
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // A portable copy keeps temp off C:. Safe here: before any thread spawns.
    if let Some(temp) = settings::temp_dir() {
        std::fs::create_dir_all(&temp).ok();
        unsafe {
            std::env::set_var("TMP", &temp);
            std::env::set_var("TEMP", &temp);
        }
    }

    // Reads the settings and loads every payload while the window and WebView2 start; the
    // first command waits for it only if it is still running.
    std::thread::spawn(|| {
        commands::preload();
    });

    let run = tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::app_state,
            addons::addons,
            commands::overview,
            roster::roster,
            commands::stats,
            run::run_summary,
            run::take_wow_run,
            commands::heatmap,
            commands::timeline,
            commands::sessions,
            commands::session_cards,
            commands::map_list,
            commands::map_data,
            commands::map_image,
            commands::map_labels,
            commands::map_layers,
            mapart::map_art_status,
            mapart::set_map_art,
            mapart::map_image_openmap,
            mapart::map_image_dark,
            mapart::open_openmap_page,
            commands::charts,
            commands::trends,
            loot::loot,
            talents::talents,
            legacy::legacy,
            items::items,
            lifetime::lifetime,
            ops::compress_export,
            ops::compress_logs,
            ops::toggle_watch,
            ops::set_setup_done,
            ops::sizes,
            ops::wow_clients,
            ops::wow_folders,
            ops::get_settings,
            ops::set_settings,
            browse::browse_wow,
            ops::save_view,
            ops::set_open_with_wow,
            ops::set_group_by_account,
            ops::set_session_mode,
            ops::session_start,
            ops::session_stop,
            updater::update_info,
            updater::update_check,
            updater::update_install,
            updater::restart_rebuilt,
            updater::set_track,
            updater::release_notes,
            addon_sync::addon_status,
            addon_sync::set_keep_addon,
            addon_sync::addon_install,
            upload::upload_get,
            upload::upload_set_enabled,
            upload::upload_set_server,
            upload::upload_set_group,
            upload::upload_remove_group,
            upload::upload_set_character,
            upload::upload_delete_all,
            upload::upload_now,
            upload::upload_linked,
            upload::upload_link,
            upload::session_on_web,
            upload::open_session_on_web,
            classpage::class_icon,
            classpage::class_page_get,
            classpage::class_page_open,
            feats::feats_get,
            feats::feats_seen,
        ])
        .setup(move |app| {
            let mut window = tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("Scarlet")
            // The header in ui/ is the title bar: it drags, and holds the window buttons.
            // The shadow keeps the edge visible; the frame stays resizable.
            .decorations(false)
            .shadow(true)
            .background_color(tauri::window::Color(0x14, 0x14, 0x16, 255))
            .inner_size(1280.0, 840.0)
            .min_inner_size(960.0, 620.0);
            if let Some(dir) = settings::webview_data() {
                std::fs::create_dir_all(&dir).ok();
                window = window.data_directory(dir);
            }
            window.build()?;

            scarlet::upload::api::set_agent(
                "scarlet-app",
                &updater::version(app.handle()).to_string(),
            );
            settings::ensure_saved();
            let s = commands::state(app.handle())
                .settings
                .lock()
                .unwrap()
                .clone();
            scarlet::overview::set_group_by_account(s.group_by_account);
            upload::init_web();
            if s.watch {
                watch::set_watch(app.handle(), true);
            }
            if !s.wow.is_dir() {
                discover::spawn(app.handle().clone());
            }
            updater::spawn(app.handle().clone());
            {
                let app = app.handle().clone();
                std::thread::spawn(move || run::watch_window(app));
            }
            upload::start_if_on_after_launch(app.handle());
            // Keeps the startup entry pointing at this exe and the watcher running after
            // a rebuild or a kill.
            if s.open_with_wow {
                let _ = launch::set_registered(true);
                launch::spawn_watcher();
            }
            Ok(())
        })
        .run(tauri::generate_context!());
    // No console in release builds: a startup failure goes to scarlet.log next to settings.json,
    // and into a message box, since a PC without the WebView2 runtime has no window to show it.
    if let Err(e) = run {
        let text = format!("Scarlet failed to start: {e}\n");
        let log = settings::log_path();
        if let Some(dir) = log.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if std::fs::write(&log, &text).is_err() {
            let _ = std::fs::write(std::env::temp_dir().join("scarlet.log"), &text);
        }
        launch::message_box(
            "Scarlet",
            &format!(
                "{text}\nIf the Microsoft Edge WebView2 Runtime is not installed, install it and start Scarlet again."
            ),
        );
    }
}

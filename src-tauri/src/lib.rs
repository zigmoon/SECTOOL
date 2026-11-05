//! SECTOOL — application de bureau (Tauri).
//!
//! Le front-end Astro vit dans `../app`. Toute la logique système est en Rust :
//! le webview n'a accès qu'aux commandes déclarées ici.

pub mod commands;
pub mod state;
pub mod worker;

use std::sync::Arc;

use tauri::Manager;

use crate::state::AppState;

/// Point d'entrée commun au binaire et aux tests.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = Arc::new(AppState::new());

    tauri::Builder::default()
        .manage(app_state.clone())
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::get_devices,
            commands::get_alerts,
            commands::clear_alerts,
            commands::scan_now,
            commands::get_settings,
            commands::save_settings_cmd,
            commands::get_rules,
            commands::get_firewall_status,
            commands::preview_rules,
            commands::save_rule,
            commands::delete_rule,
            commands::toggle_rule,
            commands::apply_rules,
            commands::flush_rules,
            commands::set_device_blocked,
            commands::is_elevated,
            commands::export_report,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            worker::spawn(handle, app_state.clone());
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("échec au démarrage de SECTOOL");
}

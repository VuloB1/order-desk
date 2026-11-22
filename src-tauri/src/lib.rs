mod backup;
mod commands;
mod db;
mod domain;
mod error;
mod export;
mod import;
mod platform;
mod vault;

use std::sync::Mutex;
use tauri::{Emitter, Manager, RunEvent};

#[tauri::command]
fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            app.manage(commands::Db(Mutex::new(db::open(
                &dir.join("business.db"),
            )?)));

            app.manage(vault::Vault::new(dir, vault::crypto::KDF));

            let handle = app.handle().clone();
            // Primero se destruye la clave en memoria; después se avisa a la UI para vaciar su estado.
            platform::session_lock::watch(move || {
                handle.state::<vault::Vault>().lock();
                let _ = handle.emit("private-lock", ());
            })?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_version,
            commands::list_products,
            commands::create_product,
            commands::update_product,
            commands::delete_product,
            commands::reorder_products,
            commands::delete_discount_code,
            commands::list_discount_codes,
            commands::create_discount_code,
            commands::update_discount_code,
            commands::list_orders,
            commands::get_order,
            commands::top_products,
            commands::create_order,
            commands::update_order,
            commands::change_order_status,
            commands::set_shipment,
            commands::delete_order,
            commands::last_order_number,
            commands::vault::vault_status,
            commands::vault::vault_create,
            commands::vault::vault_unlock,
            commands::vault::vault_reset_password,
            commands::vault::vault_change_password,
            commands::vault::vault_lock,
            commands::vault::get_private_data,
            commands::vault::set_private_data,
            commands::export_orders,
            commands::import_preview,
            commands::import_apply,
            commands::backup::backup_create,
            commands::backup::backup_restore,
            commands::update::check_update,
            commands::update::install_update,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // Al cerrar la aplicación la sección privada queda bloqueada.
            if let RunEvent::Exit = event {
                app.state::<vault::Vault>().lock();
            }
        });
}

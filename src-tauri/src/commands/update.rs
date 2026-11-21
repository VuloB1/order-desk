//! Actualizaciones: se comprueban y se instalan desde Rust (la interfaz no tiene permisos del plugin).
//! Cada paquete lleva una firma que se verifica con la clave pública de `tauri.conf.json`.
use crate::error::{AppError, Result};
use crate::vault::Vault;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub notes: Option<String>,
}

fn net_err<E>(_: E) -> AppError {
    // Sin detalles: pueden incluir URLs o rutas.
    AppError::Io("No se pudo consultar las actualizaciones (¿hay conexión a internet?)".into())
}

async fn available(app: &AppHandle) -> Result<Option<tauri_plugin_updater::Update>> {
    app.updater()
        .map_err(net_err)?
        .check()
        .await
        .map_err(net_err)
}

#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<Option<UpdateInfo>> {
    Ok(available(&app).await?.map(|u| UpdateInfo {
        version: u.version,
        notes: u.body,
    }))
}

/// Descarga e instala la última versión. En Windows el instalador cierra la app y la reabre.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<()> {
    let Some(update) = available(&app).await? else {
        return Err(AppError::NotFound(
            "No hay actualizaciones disponibles".into(),
        ));
    };
    // El instalador termina el proceso sin pasar por RunEvent::Exit: se bloquea antes.
    app.state::<Vault>().lock();
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|_| AppError::Io("No se pudo instalar la actualización".into()))
}

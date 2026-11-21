//! Copias de seguridad cifradas: crear (requiere la sección privada desbloqueada, porque la clave
//! sale de la DEK) y restaurar (con la contraseña o clave de recuperación de la propia copia).
use super::{Db, save_dialog};
use crate::backup::{self, Secret};
use crate::error::{AppError, Result};
use crate::vault::Vault;
use rusqlite::Connection;
use tauri::State;
use zeroize::Zeroizing;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupResult {
    pub path: String,
    pub bytes: usize,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreResult {
    pub orders: i64,
}

#[tauri::command]
pub async fn backup_create(
    window: tauri::WebviewWindow,
    db: State<'_, Db>,
    vault: State<'_, Vault>,
    file_name: String,
) -> Result<Option<BackupResult>> {
    let data = {
        // Con ambos cierres tomados no hay escrituras a medias mientras se leen los ficheros.
        let _db = db.conn();
        vault.with_dek(|dek| {
            let (business, private) = backup::snapshot(vault.dir())?;
            backup::create(dek, &vault.vault_bytes()?, &business, &private)
        })?
    };
    let bytes = data.len();
    Ok(save_dialog(
        window,
        "Guardar copia de seguridad",
        file_name,
        "Copia de Order Desk",
        "odbak",
        data,
    )
    .await?
    .map(|path| BackupResult { path, bytes }))
}

/// Elige una copia, la descifra y reemplaza los datos actuales (que se conservan como
/// `*.before-restore`). `kind`: "password" o "recovery". La sección privada queda bloqueada.
#[tauri::command]
pub async fn backup_restore(
    window: tauri::WebviewWindow,
    db: State<'_, Db>,
    vault: State<'_, Vault>,
    kind: String,
    secret: String,
) -> Result<Option<RestoreResult>> {
    let recovery = match kind.as_str() {
        "password" => false,
        "recovery" => true,
        _ => return Err(AppError::Validation("Tipo de credencial no válido".into())),
    };
    let secret = Zeroizing::new(secret);

    let picked = tauri::async_runtime::spawn_blocking(move || {
        rfd::FileDialog::new()
            .set_parent(&window)
            .set_title("Elegir copia de seguridad")
            .add_filter("Copia de Order Desk", &["odbak"])
            .pick_file()
            .map(std::fs::read)
    })
    .await
    .map_err(|_| AppError::Io("La operación se interrumpió".into()))?;
    let Some(bytes) = picked else { return Ok(None) };
    let bytes = bytes.map_err(|_| AppError::Io("No se pudo leer el archivo".into()))?;

    // Argon2 tarda una fracción de segundo: fuera del hilo de la interfaz.
    let (opened, dek) = tauri::async_runtime::spawn_blocking(move || {
        let s = if recovery {
            Secret::Recovery(&secret)
        } else {
            Secret::Password(&secret)
        };
        backup::open(&bytes, s)
    })
    .await
    .map_err(|_| AppError::Io("La operación se interrumpió".into()))??;

    let dir = vault.dir().to_path_buf();
    let mut conn = db.conn();
    // Cerrar las bases para poder renombrar sus ficheros en Windows.
    let _ = std::mem::replace(&mut *conn, Connection::open_in_memory()?).close();
    vault.lock();
    let restored = backup::restore_into(&dir, &opened, &dek);
    // Se reabre siempre: la copia restaurada o, si falló, los datos de antes.
    *conn = crate::db::open(&dir.join("business.db")).map_err(|_| {
        AppError::Io(
            "No se pudo reabrir la base de datos: cierra y vuelve a abrir la aplicación".into(),
        )
    })?;
    restored?;
    let orders = conn.query_row("SELECT count(*) FROM orders", [], |r| r.get::<_, i64>(0))?;
    Ok(Some(RestoreResult { orders }))
}

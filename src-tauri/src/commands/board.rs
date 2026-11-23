//! Tablero público: ajustes (dirección del repositorio) y publicación con un clic.
use super::Db;
use crate::domain::order::OrderFilter;
use crate::error::{AppError, Result};
use crate::{export, publish};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct BoardSettings {
    pub repo_url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishResult {
    /// `false` = el tablero ya estaba al día (no hubo nada que subir).
    pub changed: bool,
    pub pages_url: Option<String>,
}

fn data_dir(app: &AppHandle) -> Result<PathBuf> {
    app.path()
        .app_data_dir()
        .map_err(|_| AppError::Io("No se encontró la carpeta de datos".into()))
}

fn load(app: &AppHandle) -> Result<BoardSettings> {
    let file = data_dir(app)?.join("board.json");
    Ok(std::fs::read(file)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default())
}

#[tauri::command]
pub fn get_board_settings(app: AppHandle) -> Result<BoardSettings> {
    load(&app)
}

/// Guarda la dirección del repositorio (vacía = sin configurar). No contiene secretos.
#[tauri::command]
pub fn set_board_settings(app: AppHandle, repo_url: String) -> Result<BoardSettings> {
    let repo_url = if repo_url.trim().is_empty() {
        String::new()
    } else {
        publish::validate_url(&repo_url)?
    };
    let s = BoardSettings { repo_url };
    let bytes =
        serde_json::to_vec(&s).map_err(|_| AppError::Io("No se pudo guardar el ajuste".into()))?;
    std::fs::write(data_dir(&app)?.join("board.json"), bytes)
        .map_err(|_| AppError::Io("No se pudo guardar el ajuste".into()))?;
    Ok(s)
}

/// Genera el tablero con los pedidos actuales (solo campos públicos) y lo sube al repositorio configurado.
#[tauri::command]
pub async fn publish_board(app: AppHandle, db: State<'_, Db>) -> Result<PublishResult> {
    let url = load(&app)?.repo_url;
    if url.is_empty() {
        return Err(AppError::Validation(
            "Configura primero el repositorio del tablero".into(),
        ));
    }
    let html = {
        let c = db.conn();
        let today: String = c.query_row("SELECT date('now','localtime')", [], |r| r.get(0))?;
        export::board::render(&export::collect(&c, &OrderFilter::default())?, &today)
    };
    let dir = data_dir(&app)?.join("board");
    let changed = {
        let url = url.clone();
        tauri::async_runtime::spawn_blocking(move || publish::publish(&dir, &url, &html, "https"))
            .await
            .map_err(|_| AppError::Io("La operación se interrumpió".into()))??
    };
    Ok(PublishResult {
        changed,
        pages_url: publish::pages_url(&url),
    })
}

pub mod backup;
pub mod update;
pub mod vault;

use crate::db::{orders, private};
use crate::domain::order::*;
use crate::error::{AppError, Result};
use crate::export::{self, Format};
use crate::vault::Vault;
use rusqlite::Connection;
use std::sync::{Mutex, MutexGuard};
use tauri::State;

pub struct Db(pub Mutex<Connection>);

impl Db {
    fn conn(&self) -> MutexGuard<'_, Connection> {
        // Un pánico previo no deja la base inconsistente (cada operación es una transacción).
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[tauri::command]
pub fn list_products(db: State<Db>) -> Result<Vec<Product>> {
    orders::list_products(&db.conn())
}

#[tauri::command]
pub fn create_product(db: State<Db>, name: String, unit_price_cents: i64) -> Result<Product> {
    orders::create_product(&db.conn(), &name, unit_price_cents)
}

#[tauri::command]
pub fn update_product(
    db: State<Db>,
    id: String,
    name: String,
    unit_price_cents: i64,
    active: bool,
) -> Result<Product> {
    orders::update_product(&db.conn(), &id, &name, unit_price_cents, active)
}

#[tauri::command]
pub fn delete_product(db: State<Db>, id: String) -> Result<()> {
    orders::delete_product(&db.conn(), &id)
}

#[tauri::command]
pub fn reorder_products(db: State<Db>, ids: Vec<String>) -> Result<()> {
    orders::reorder_products(&mut db.conn(), &ids)
}

#[tauri::command]
pub fn list_discount_codes(db: State<Db>) -> Result<Vec<DiscountCode>> {
    orders::list_discount_codes(&db.conn())
}

#[tauri::command]
pub fn create_discount_code(
    db: State<Db>,
    code: String,
    kind: String,
    value: i64,
) -> Result<DiscountCode> {
    orders::create_discount_code(&db.conn(), &code, &kind, value)
}

#[tauri::command]
pub fn update_discount_code(
    db: State<Db>,
    id: String,
    code: String,
    kind: String,
    value: i64,
    active: bool,
) -> Result<DiscountCode> {
    orders::update_discount_code(&db.conn(), &id, &code, &kind, value, active)
}

#[tauri::command]
pub fn delete_discount_code(db: State<Db>, id: String) -> Result<()> {
    orders::delete_discount_code(&db.conn(), &id)
}

/// Con la sección privada desbloqueada, la búsqueda también mira nombre y RUT (en memoria; nada
/// de eso llega a business.db). Bloqueada, solo busca por número de pedido.
fn with_private_matches(vault: &Vault, filter: Option<OrderFilter>) -> OrderFilter {
    let mut f = filter.unwrap_or_default();
    if let Some(term) = f.search.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        f.private_ids = vault.with(|c| private::search(c, term)).unwrap_or_default();
    }
    f
}

#[tauri::command]
pub fn list_orders(
    db: State<Db>,
    vault: State<Vault>,
    filter: Option<OrderFilter>,
) -> Result<Vec<OrderSummary>> {
    orders::list_orders(&db.conn(), &with_private_matches(&vault, filter))
}

#[tauri::command]
pub fn top_products(db: State<Db>, limit: i64) -> Result<Vec<ProductTotal>> {
    orders::top_products(&db.conn(), limit)
}

#[tauri::command]
pub fn get_order(db: State<Db>, id: String) -> Result<OrderDetail> {
    orders::get_order(&db.conn(), &id)
}

#[tauri::command]
pub fn create_order(db: State<Db>, input: OrderInput) -> Result<OrderDetail> {
    orders::create_order(&mut db.conn(), input)
}

#[tauri::command]
pub fn update_order(db: State<Db>, id: String, input: OrderInput) -> Result<OrderDetail> {
    orders::update_order(&mut db.conn(), &id, input)
}

#[tauri::command]
pub fn change_order_status(db: State<Db>, id: String, status: String) -> Result<OrderDetail> {
    orders::change_status(&mut db.conn(), &id, &status)
}

#[tauri::command]
pub fn set_shipment(db: State<Db>, id: String, shipment: Shipment) -> Result<OrderDetail> {
    orders::set_shipment(&db.conn(), &id, shipment)
}

#[tauri::command]
pub fn last_order_number(db: State<Db>) -> Result<Option<String>> {
    orders::last_order_number(&db.conn())
}

#[tauri::command]
pub fn delete_order(db: State<Db>, vault: State<Vault>, id: String) -> Result<()> {
    orders::delete_order(&db.conn(), &id)?;
    // Bloqueado no se puede borrar aquí: los datos huérfanos se purgan al desbloquear.
    let _ = vault.with(|c| private::delete(c, &id));
    Ok(())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub path: String,
    pub count: usize,
}

/// Exporta los pedidos que cumplen el filtro. Lee solo de business.db; el fichero lo elige el
/// usuario en el cuadro nativo de Windows. `None` = cancelado.
#[tauri::command]
pub async fn export_orders(
    window: tauri::WebviewWindow,
    db: State<'_, Db>,
    vault: State<'_, Vault>,
    format: String,
    filter: Option<OrderFilter>,
    file_name: String,
) -> Result<Option<ExportResult>> {
    let format = Format::parse(&format)?;
    let rows = export::collect(&db.conn(), &with_private_matches(&vault, filter))?;
    let bytes = export::render(format, &rows)?;
    let count = rows.len();
    let (label, ext) = match format {
        Format::Json => ("JSON", "json"),
        Format::Csv => ("CSV (Excel)", "csv"),
    };
    Ok(
        save_dialog(window, "Exportar pedidos", file_name, label, ext, bytes)
            .await?
            .map(|path| ExportResult { path, count }),
    )
}

/// Cuadro "Guardar como" nativo y escritura del fichero. `None` = cancelado. Bloquea, por eso va en
/// un hilo aparte para no congelar la interfaz.
pub(crate) async fn save_dialog(
    window: tauri::WebviewWindow,
    title: &'static str,
    file_name: String,
    label: &'static str,
    ext: &'static str,
    bytes: Vec<u8>,
) -> Result<Option<String>> {
    tauri::async_runtime::spawn_blocking(move || {
        let Some(path) = rfd::FileDialog::new()
            .set_parent(&window)
            .set_title(title)
            .set_file_name(file_name.as_str())
            .add_filter(label, &[ext])
            .save_file()
        else {
            return Ok(None);
        };
        std::fs::write(&path, bytes)
            .map_err(|_| AppError::Io("No se pudo guardar el archivo".into()))?;
        Ok(Some(path.display().to_string()))
    })
    .await
    .map_err(|_| AppError::Io("La operación se interrumpió".into()))?
}

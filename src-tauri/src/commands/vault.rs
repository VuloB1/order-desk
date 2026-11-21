//! Comandos de la sección privada. La UI solo envía contraseñas y recibe estado o datos;
//! las claves nunca salen de Rust. La clave de recuperación se devuelve una única vez, al crear.
use super::Db;
use crate::db::private::{self, PrivateData};
use crate::error::{AppError, Result};
use crate::vault::Vault;
use tauri::State;

#[tauri::command]
pub fn vault_status(vault: State<Vault>) -> &'static str {
    vault.status()
}

#[tauri::command]
pub fn vault_create(vault: State<Vault>, password: String) -> Result<String> {
    vault.create(&password).map(|k| k.to_string())
}

#[tauri::command]
pub fn vault_unlock(vault: State<Vault>, db: State<Db>, password: String) -> Result<()> {
    vault.unlock(&password)?;
    purge(&vault, &db)
}

#[tauri::command]
pub fn vault_reset_password(
    vault: State<Vault>,
    db: State<Db>,
    recovery_key: String,
    new_password: String,
) -> Result<()> {
    vault.reset_password(&recovery_key, &new_password)?;
    purge(&vault, &db)
}

#[tauri::command]
pub fn vault_change_password(
    vault: State<Vault>,
    old_password: String,
    new_password: String,
) -> Result<()> {
    vault.change_password(&old_password, &new_password)
}

#[tauri::command]
pub fn vault_lock(vault: State<Vault>) {
    vault.lock();
}

#[tauri::command]
pub fn get_private_data(vault: State<Vault>, order_id: String) -> Result<Option<PrivateData>> {
    vault.with(|c| private::get(c, &order_id))
}

#[tauri::command]
pub fn set_private_data(
    vault: State<Vault>,
    db: State<Db>,
    order_id: String,
    data: PrivateData,
) -> Result<()> {
    if !private::order_ids(&db.conn())?.contains(&order_id) {
        return Err(AppError::NotFound("El pedido no existe".into()));
    }
    vault.with(|c| private::set(c, &order_id, data))
}

/// Al desbloquear se eliminan los datos de pedidos borrados mientras estaba bloqueado.
fn purge(vault: &Vault, db: &Db) -> Result<()> {
    let keep = private::order_ids(&db.conn())?;
    vault.with(|c| private::purge_orphans(c, &keep)).map(|_| ())
}

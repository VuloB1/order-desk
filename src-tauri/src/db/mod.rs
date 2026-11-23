pub mod orders;
pub mod private;

use crate::error::{AppError, Result};
use rusqlite::{Connection, OptionalExtension};
use std::path::Path;

/// Migraciones en orden; la posición + 1 es el `PRAGMA user_version` resultante.
const MIGRATIONS: &[&str] = &[
    include_str!("migrations/0001_init.sql"),
    include_str!("migrations/0002_discounts_and_order.sql"),
    include_str!("migrations/0003_destination.sql"),
    include_str!("migrations/0004_drop_sku_usage.sql"),
    include_str!("migrations/0005_invoice.sql"),
    include_str!("migrations/0006_new_statuses.sql"),
];

pub fn migrate(c: &mut Connection) -> Result<()> {
    migrate_with(c, MIGRATIONS)
}

pub fn migrate_with(c: &mut Connection, migrations: &[&str]) -> Result<()> {
    let current: i64 = c.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    let current = usize::try_from(current).unwrap_or(usize::MAX);
    if current > migrations.len() {
        return Err(AppError::Validation(
            "La base de datos es de una versión más nueva que la aplicación".into(),
        ));
    }
    // Reconstruir una tabla (SQLite no altera un CHECK) exige foreign_keys=OFF, y ese ajuste no
    // se puede cambiar dentro de una transacción: se apaga aquí y se restaura al terminar.
    let fk: bool = c.query_row("PRAGMA foreign_keys", [], |r| r.get(0))?;
    c.pragma_update(None, "foreign_keys", false)?;
    let run = (|| -> Result<()> {
        for (i, sql) in migrations.iter().enumerate().skip(current) {
            let tx = c.transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", (i + 1) as i64)?;
            tx.commit()?;
        }
        Ok(())
    })();
    c.pragma_update(None, "foreign_keys", fk)?;
    run?;
    // Con las claves ajenas apagadas nadie las vigila: se comprueba que la migración no dejó huérfanos.
    let orphan: Option<String> = c
        .query_row("PRAGMA foreign_key_check", [], |r| r.get(0))
        .optional()?;
    if orphan.is_some() {
        return Err(AppError::Validation(
            "La migración dejó datos huérfanos".into(),
        ));
    }
    Ok(())
}

fn init(c: &mut Connection) -> Result<()> {
    c.pragma_update(None, "foreign_keys", true)?;
    migrate(c)
}

pub fn open(path: &Path) -> Result<Connection> {
    let mut c = Connection::open(path)?;
    init(&mut c)?;
    Ok(c)
}

#[cfg(test)]
pub fn open_in_memory() -> Connection {
    let mut c = Connection::open_in_memory().expect("memoria");
    init(&mut c).expect("migraciones");
    c
}

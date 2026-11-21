//! private.db (SQLCipher): datos personales de cada pedido. Nada de esto toca business.db.
use super::migrate_with;
use crate::error::{AppError, Result};
use crate::vault::crypto::{Key, auth_error, hex};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use zeroize::Zeroizing;

const MIGRATIONS: &[&str] = &[
    include_str!("private_migrations/0001_private.sql"),
    include_str!("private_migrations/0002_shipping_fields.sql"),
    include_str!("private_migrations/0003_tax_fields.sql"),
];

const MAX_NAME: usize = 200;
const MAX_EMAIL: usize = 254;
const MAX_PHONE: usize = 50;
const MAX_ADDRESS: usize = 500;
const MAX_PLACE: usize = 120; // país, región, ciudad
const MAX_POSTAL: usize = 20;
const MAX_NOTES: usize = 500;
const MAX_BUSINESS: usize = 200; // razón social y giro

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivateData {
    pub name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
    pub postal_code: Option<String>,
    pub delivery_notes: Option<String>,
    pub rut: Option<String>,
    pub business_name: Option<String>,
    pub business_activity: Option<String>,
}

/// Abre private.db con la DEK (clave en bruto de SQLCipher) y aplica las migraciones.
pub fn open(path: &Path, dek: &Key) -> Result<Connection> {
    let mut c = Connection::open(path)?;
    let hex_key = Zeroizing::new(hex(&**dek));
    let pragma = Zeroizing::new(format!("PRAGMA key = \"x'{}'\";", &*hex_key));
    c.execute_batch(&pragma)?;
    // Con clave equivocada (o fichero ajeno) esta lectura falla.
    c.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
        r.get::<_, i64>(0)
    })
    .map_err(|_| auth_error())?;
    migrate_with(&mut c, MIGRATIONS)?;
    Ok(c)
}

/// RUT chileno: acepta puntos, espacios y guion; valida el dígito verificador y lo guarda como `12345678-5`.
fn clean_rut(v: Option<String>) -> Result<Option<String>> {
    let Some(raw) = clean(v, 20, "RUT")? else {
        return Ok(None);
    };
    let compact: String = raw
        .chars()
        .filter(|c| !matches!(c, '.' | ' ' | '-'))
        .collect::<String>()
        .to_uppercase();
    let bad = || AppError::Validation("RUT no válido (ejemplo: 12.345.678-5)".into());
    let (body, dv) = compact.split_at(compact.len().saturating_sub(1));
    if !(1..=8).contains(&body.len()) || !body.bytes().all(|b| b.is_ascii_digit()) {
        return Err(bad());
    }
    let sum: u32 = body
        .bytes()
        .rev()
        .zip([2, 3, 4, 5, 6, 7].iter().cycle())
        .map(|(b, f)| u32::from(b - b'0') * f)
        .sum();
    let expected = match 11 - sum % 11 {
        11 => "0".to_string(),
        10 => "K".to_string(),
        n => n.to_string(),
    };
    if dv != expected {
        return Err(bad());
    }
    Ok(Some(format!("{body}-{dv}")))
}

fn clean(v: Option<String>, max: usize, label: &str) -> Result<Option<String>> {
    let v = v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    if v.as_ref().is_some_and(|s| s.chars().count() > max) {
        return Err(AppError::Validation(format!(
            "{label}: máximo {max} caracteres"
        )));
    }
    Ok(v)
}

pub fn get(c: &Connection, order_id: &str) -> Result<Option<PrivateData>> {
    Ok(c.query_row(
        "SELECT name, email, phone, address, country, region, city, postal_code, delivery_notes,
                rut, business_name, business_activity
         FROM order_private WHERE order_id = ?1",
        [order_id],
        |r| {
            Ok(PrivateData {
                name: r.get(0)?,
                email: r.get(1)?,
                phone: r.get(2)?,
                address: r.get(3)?,
                country: r.get(4)?,
                region: r.get(5)?,
                city: r.get(6)?,
                postal_code: r.get(7)?,
                delivery_notes: r.get(8)?,
                rut: r.get(9)?,
                business_name: r.get(10)?,
                business_activity: r.get(11)?,
            })
        },
    )
    .optional()?)
}

/// Guarda los datos; si todos los campos quedan vacíos, borra la fila.
pub fn set(c: &Connection, order_id: &str, d: PrivateData) -> Result<()> {
    let d = PrivateData {
        name: clean(d.name, MAX_NAME, "Nombre")?,
        email: clean(d.email, MAX_EMAIL, "Correo")?,
        phone: clean(d.phone, MAX_PHONE, "Teléfono")?,
        address: clean(d.address, MAX_ADDRESS, "Dirección")?,
        country: clean(d.country, MAX_PLACE, "País")?,
        region: clean(d.region, MAX_PLACE, "Región o zona")?,
        city: clean(d.city, MAX_PLACE, "Ciudad o comuna")?,
        postal_code: clean(d.postal_code, MAX_POSTAL, "Código postal")?,
        delivery_notes: clean(d.delivery_notes, MAX_NOTES, "Indicaciones de entrega")?,
        rut: clean_rut(d.rut)?,
        business_name: clean(d.business_name, MAX_BUSINESS, "Razón social")?,
        business_activity: clean(d.business_activity, MAX_BUSINESS, "Giro")?,
    };
    if d == PrivateData::default() {
        return delete(c, order_id);
    }
    c.execute(
        "INSERT INTO order_private
            (order_id, name, email, phone, address, country, region, city, postal_code, delivery_notes,
             rut, business_name, business_activity)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
         ON CONFLICT(order_id) DO UPDATE SET
            name = ?2, email = ?3, phone = ?4, address = ?5, country = ?6, region = ?7, city = ?8,
            postal_code = ?9, delivery_notes = ?10, rut = ?11, business_name = ?12,
            business_activity = ?13, updated_at = datetime('now')",
        params![
            order_id, d.name, d.email, d.phone, d.address, d.country, d.region, d.city,
            d.postal_code, d.delivery_notes, d.rut, d.business_name, d.business_activity
        ],
    )?;
    Ok(())
}

pub fn delete(c: &Connection, order_id: &str) -> Result<()> {
    c.execute("DELETE FROM order_private WHERE order_id = ?1", [order_id])?;
    Ok(())
}

/// Pedidos cuyo nombre contiene `term` o cuyo RUT lo contiene (sin puntos ni guion).
pub fn search(c: &Connection, term: &str) -> Result<Vec<String>> {
    let rut: String = term
        .chars()
        .filter(|ch| !matches!(ch, '.' | ' ' | '-'))
        .collect::<String>()
        .to_uppercase();
    Ok(c.prepare(
        "SELECT order_id FROM order_private
         WHERE name LIKE '%' || ?1 || '%' ESCAPE '\\'
            OR (?2 <> '' AND replace(rut, '-', '') LIKE '%' || ?2 || '%' ESCAPE '\\')",
    )?
    .query_map(
        params![
            super::orders::like_escape(term),
            super::orders::like_escape(&rut)
        ],
        |r| r.get(0),
    )?
    .collect::<std::result::Result<_, _>>()?)
}

/// Borra los datos de pedidos que ya no existen (p. ej. eliminados mientras estaba bloqueado).
pub fn purge_orphans(c: &Connection, keep: &HashSet<String>) -> Result<usize> {
    let ids: Vec<String> = c
        .prepare("SELECT order_id FROM order_private")?
        .query_map([], |r| r.get(0))?
        .collect::<std::result::Result<_, _>>()?;
    let mut n = 0;
    for id in ids.iter().filter(|id| !keep.contains(*id)) {
        delete(c, id)?;
        n += 1;
    }
    Ok(n)
}

/// IDs de pedidos de business.db (para purgar huérfanos y validar existencia).
pub fn order_ids(business: &Connection) -> Result<HashSet<String>> {
    Ok(business
        .prepare("SELECT id FROM orders")?
        .query_map([], |r| r.get(0))?
        .collect::<std::result::Result<_, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rut(s: &str) -> Result<Option<String>> {
        clean_rut(Some(s.into()))
    }

    #[test]
    fn search_by_name_or_rut() {
        let mut c = Connection::open_in_memory().unwrap();
        migrate_with(&mut c, MIGRATIONS).unwrap();
        let put = |id: &str, name: &str, rut: &str| {
            set(
                &c,
                id,
                PrivateData {
                    name: Some(name.into()),
                    rut: Some(rut.into()),
                    ..Default::default()
                },
            )
            .unwrap();
        };
        put("a", "Persona de Prueba", "12.345.678-5");
        put("b", "Otra Persona", "11111111-1");
        let ids = |t: &str| {
            let mut v = search(&c, t).unwrap();
            v.sort();
            v
        };
        assert_eq!(ids("persona"), ["a", "b"]);
        assert_eq!(ids("de prue"), ["a"]);
        assert_eq!(ids("12.345.678-5"), ["a"]);
        assert_eq!(ids("2345"), ["a"]);
        assert!(ids("%").is_empty()); // los comodines no son comodines
        assert!(ids("zzz").is_empty());
    }

    #[test]
    fn rut_is_validated_and_normalized() {
        assert_eq!(rut("12.345.678-5").unwrap().as_deref(), Some("12345678-5"));
        assert_eq!(rut(" 11111111 1 ").unwrap().as_deref(), Some("11111111-1"));
        assert_eq!(rut("7.654.321-6").unwrap().as_deref(), Some("7654321-6"));
        assert_eq!(rut("6.000.000-k").unwrap().as_deref(), Some("6000000-K"));
        assert_eq!(rut("").unwrap(), None);
        for bad in ["12.345.678-4", "abc", "1", "123456789-0", "12.345.678-"] {
            assert!(rut(bad).is_err(), "{bad}");
        }
    }
}

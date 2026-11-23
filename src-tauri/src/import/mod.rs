//! Importación de pedidos desde un Excel (.xlsx) con columnas «Pedidos», «Kit» y «País».
//! Solo toca datos públicos (business.db). Un pedido que ya existe solo cambia de destino y, si
//! tiene una única línea, de producto; estado, fechas, precios, envío y datos privados no se tocan.

use crate::db::orders;
use crate::domain::order::{Item, OrderFilter, OrderInput};
use crate::error::{AppError, Result};
use calamine::{Data, Reader, Xlsx, open_workbook};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImportRow {
    /// Fila del Excel (empieza en 1), para localizar avisos.
    pub row: usize,
    pub number: String,
    pub kit: String,
    pub country: String,
    pub destination: String,
    /// `new`, `update`, `same` o `skip`.
    pub action: String,
    pub note: Option<String>,
}

#[derive(Serialize, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub created: usize,
    pub updated: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

fn fold(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .replace(['á', 'à'], "a")
        .replace('é', "e")
        .replace('í', "i")
        .replace('ó', "o")
        .replace(['ú', 'ü'], "u")
}

/// `1042` o `#1042` → `#1042`. Cualquier otro texto se conserva tal cual (el número es texto).
fn normalize_number(s: &str) -> String {
    let s = s.trim();
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) {
        format!("#{s}")
    } else {
        s.to_string()
    }
}

/// El país del Excel viene con erratas («Argetina», «Chile mayo»): todo lo que menciona Chile es
/// `chile`, el resto es `foreign`. Sin país se asume `chile` y se avisa.
fn destination_for(country: &str) -> (&'static str, Option<&'static str>) {
    let c = fold(country);
    if c.is_empty() {
        ("chile", Some("Sin país: se asume Chile"))
    } else if c.contains("chile") {
        ("chile", None)
    } else {
        ("foreign", None)
    }
}

fn cell_text(d: &Data) -> String {
    match d {
        Data::Empty => String::new(),
        Data::Float(f) if f.fract() == 0.0 => format!("{}", *f as i64),
        other => other.to_string().trim().to_string(),
    }
}

/// Lee la primera hoja. La cabecera es la primera fila (entre las 20 primeras) que contiene
/// «Pedidos», «Kit» y «País»; el resto de columnas se ignoran. Devuelve (fila, número, kit, país).
pub fn read_sheet(path: &Path) -> Result<Vec<(usize, String, String, String)>> {
    let unreadable =
        |_| AppError::Validation("No se pudo leer el Excel (debe ser un .xlsx válido)".into());
    let mut wb: Xlsx<_> = open_workbook(path).map_err(unreadable)?;
    let range = wb
        .worksheet_range_at(0)
        .ok_or_else(|| AppError::Validation("El Excel no tiene hojas".into()))?
        .map_err(unreadable)?;
    let rows: Vec<Vec<String>> = range
        .rows()
        .map(|r| r.iter().map(cell_text).collect())
        .collect();
    let col =
        |r: &[String], names: &[&str]| r.iter().position(|c| names.contains(&fold(c).as_str()));
    let (head, (n, k, p)) = rows
        .iter()
        .take(20)
        .enumerate()
        .find_map(|(i, r)| {
            Some((
                i,
                (
                    col(r, &["pedidos", "pedido"])?,
                    col(r, &["kit"])?,
                    col(r, &["pais"])?,
                ),
            ))
        })
        .ok_or_else(|| {
            AppError::Validation("No encuentro las columnas «Pedidos», «Kit» y «País»".into())
        })?;
    // `range` empieza en la primera celda con datos: se recupera su fila real para los avisos.
    let first = range.start().map_or(0, |s| s.0 as usize);
    let get = |r: &[String], i: usize| r.get(i).cloned().unwrap_or_default();
    Ok(rows
        .iter()
        .enumerate()
        .skip(head + 1)
        .map(|(i, r)| {
            (
                first + i + 1,
                normalize_number(&get(r, n)),
                get(r, k),
                get(r, p),
            )
        })
        .filter(|(_, num, kit, country)| !(num.is_empty() && kit.is_empty() && country.is_empty()))
        .collect())
}

fn product_by_name(c: &Connection, name: &str) -> Result<Option<String>> {
    Ok(c.query_row(
        "SELECT id FROM products WHERE name = ?1 COLLATE NOCASE LIMIT 1",
        [name],
        |r| r.get(0),
    )
    .optional()?)
}

fn skip(row: usize, number: &str, kit: &str, country: &str, dest: &str, why: &str) -> ImportRow {
    ImportRow {
        row,
        number: number.into(),
        kit: kit.into(),
        country: country.into(),
        destination: dest.into(),
        action: "skip".into(),
        note: Some(why.into()),
    }
}

/// Decide qué hacer con una fila comparándola con la base de datos.
fn plan_row(
    c: &Connection,
    seen: &mut HashSet<String>,
    row: usize,
    number: &str,
    kit: &str,
    country: &str,
) -> Result<ImportRow> {
    let (dest, dest_note) = destination_for(country);
    let kit = kit.trim();
    if number.is_empty() || number.chars().count() > 64 {
        return Ok(skip(
            row,
            number,
            kit,
            country,
            dest,
            "Número de pedido vacío o demasiado largo",
        ));
    }
    if kit.is_empty() {
        return Ok(skip(row, number, kit, country, dest, "Sin kit"));
    }
    if !seen.insert(number.to_string()) {
        return Ok(skip(
            row,
            number,
            kit,
            country,
            dest,
            "Repetido en el archivo",
        ));
    }
    let existing = orders::list_orders(
        c,
        &OrderFilter {
            search: Some(number.into()),
            ..Default::default()
        },
    )?
    .into_iter()
    .find(|o| o.external_number == number);
    let mut out = ImportRow {
        row,
        number: number.into(),
        kit: kit.into(),
        country: country.into(),
        destination: dest.into(),
        action: "new".into(),
        note: dest_note.map(Into::into),
    };
    let Some(o) = existing else { return Ok(out) };
    let items = orders::get_order(c, &o.id)?.items;
    let kit_changes = match items.as_slice() {
        [one] => !one.name.eq_ignore_ascii_case(kit),
        _ => {
            if !items.iter().any(|i| i.name.eq_ignore_ascii_case(kit)) {
                out.note = Some("Tiene varios productos: solo se actualiza el destino".into());
            }
            false
        }
    };
    out.action = if kit_changes || o.destination != dest {
        "update"
    } else {
        "same"
    }
    .into();
    Ok(out)
}

pub fn preview(c: &Connection, path: &Path) -> Result<Vec<ImportRow>> {
    let mut seen = HashSet::new();
    read_sheet(path)?
        .into_iter()
        .map(|(row, n, k, p)| plan_row(c, &mut seen, row, &n, &k, &p))
        .collect()
}

/// Aplica la importación. Vuelve a decidir cada fila contra la base de datos (no se fía de la
/// acción que envía la interfaz). `status` y `date` solo se usan en los pedidos nuevos.
pub fn apply(
    c: &mut Connection,
    rows: Vec<ImportRow>,
    status: &str,
    date: &str,
) -> Result<ImportResult> {
    let mut seen = HashSet::new();
    let mut res = ImportResult::default();
    for r in rows {
        let p = plan_row(c, &mut seen, r.row, &r.number, &r.kit, &r.country)?;
        let done = match p.action.as_str() {
            "new" => create(c, &p, status, date).map(|_| res.created += 1),
            "update" => update(c, &p).map(|_| res.updated += 1),
            _ => {
                res.skipped += 1;
                continue;
            }
        };
        if let Err(e) = done {
            res.errors
                .push(format!("Fila {} ({}): {e}", p.row, p.number));
        }
    }
    Ok(res)
}

fn line(c: &Connection, kit: &str, qty: i64, price: i64) -> Result<Item> {
    Ok(Item {
        product_id: product_by_name(c, kit)?,
        name: kit.into(),
        quantity: qty,
        unit_price_cents: price,
    })
}

fn create(c: &mut Connection, p: &ImportRow, status: &str, date: &str) -> Result<()> {
    let item = line(c, &p.kit, 1, 0)?;
    orders::create_order(
        c,
        OrderInput {
            store_id: String::new(),
            external_number: p.number.clone(),
            order_date: date.into(),
            status: Some(status.into()),
            shipping_cost_cents: 0,
            discount_cents: 0,
            discount_code: None,
            destination: p.destination.clone(),
            invoice_required: false,
            invoice_number: None,
            items: vec![item],
        },
    )
    .map(|_| ())
}

fn update(c: &mut Connection, p: &ImportRow) -> Result<()> {
    let id = orders::list_orders(
        c,
        &OrderFilter {
            search: Some(p.number.clone()),
            ..Default::default()
        },
    )?
    .into_iter()
    .find(|o| o.external_number == p.number)
    .map(|o| o.id)
    .ok_or_else(|| AppError::NotFound("Pedido no encontrado".into()))?;
    let o = orders::get_order(c, &id)?;
    let items = match o.items.as_slice() {
        [one] if !one.name.eq_ignore_ascii_case(&p.kit) => {
            vec![line(c, &p.kit, one.quantity, one.unit_price_cents)?]
        }
        _ => o.items.clone(),
    };
    let s = &o.summary;
    orders::update_order(
        c,
        &id,
        OrderInput {
            store_id: String::new(),
            external_number: s.external_number.clone(),
            order_date: s.order_date.clone(),
            status: None,
            shipping_cost_cents: s.shipping_cost_cents,
            discount_cents: s.discount_cents,
            discount_code: s.discount_code.clone(),
            destination: p.destination.clone(),
            invoice_required: s.invoice_required,
            invoice_number: s.invoice_number.clone(),
            items,
        },
    )
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    fn run(c: &Connection, data: &[(&str, &str, &str)]) -> Vec<ImportRow> {
        let mut seen = HashSet::new();
        data.iter()
            .enumerate()
            .map(|(i, (n, k, p))| {
                plan_row(c, &mut seen, i + 1, &normalize_number(n), k, p).unwrap()
            })
            .collect()
    }

    #[test]
    fn numbers_and_countries_are_normalized() {
        assert_eq!(normalize_number("1042"), "#1042");
        assert_eq!(normalize_number(" #1042 "), "#1042");
        assert_eq!(normalize_number("A-7"), "A-7");
        assert_eq!(destination_for("Chile").0, "chile");
        assert_eq!(destination_for("Chile mayo").0, "chile");
        assert_eq!(destination_for("Argetina").0, "foreign");
        assert_eq!(destination_for("España").0, "foreign");
        assert_eq!(destination_for("").0, "chile");
    }

    #[test]
    fn new_orders_are_created_and_existing_ones_updated_without_losing_data() {
        let mut c = open_in_memory();
        let rows = run(
            &c,
            &[
                ("#1", "bmix6", "Chile"),
                ("#2", "bnox6", "Peru"),
                ("#1", "otro", "Chile"),
                ("#3", "", "Chile"),
            ],
        );
        assert_eq!(
            rows.iter().map(|r| r.action.as_str()).collect::<Vec<_>>(),
            ["new", "new", "skip", "skip"]
        );

        let r = apply(&mut c, rows, "shipped", "2026-01-31").unwrap();
        assert_eq!((r.created, r.updated, r.skipped), (2, 0, 2));
        let listed = orders::list_orders(&c, &OrderFilter::default()).unwrap();
        assert!(
            listed
                .iter()
                .all(|o| o.status == "shipped" && o.order_date == "2026-01-31")
        );
        assert_eq!(
            listed
                .iter()
                .find(|o| o.external_number == "#2")
                .unwrap()
                .destination,
            "foreign"
        );

        // Segunda importación: #1 cambia de país y de kit, #2 no cambia. El estado y la fecha se conservan.
        let again = run(&c, &[("#1", "icmx6", "Mexico"), ("#2", "BNOX6", "Peru")]);
        assert_eq!(
            again.iter().map(|r| r.action.as_str()).collect::<Vec<_>>(),
            ["update", "same"]
        );
        let r = apply(&mut c, again, "pending", "2030-01-01").unwrap();
        assert_eq!((r.created, r.updated, r.skipped), (0, 1, 1));
        let o = &orders::list_orders(
            &c,
            &OrderFilter {
                search: Some("#1".into()),
                ..Default::default()
            },
        )
        .unwrap()[0];
        assert_eq!(
            (
                o.status.as_str(),
                o.destination.as_str(),
                o.order_date.as_str()
            ),
            ("shipped", "foreign", "2026-01-31")
        );
        assert_eq!(orders::get_order(&c, &o.id).unwrap().items[0].name, "icmx6");
    }

    /// Manual: `ORDERDESK_XLSX=ruta.xlsx cargo test real_excel -- --ignored --nocapture`. Solo imprime recuentos.
    #[test]
    #[ignore]
    fn real_excel_is_read() {
        let path = std::env::var("ORDERDESK_XLSX").expect("define ORDERDESK_XLSX");
        let c = open_in_memory();
        let rows = preview(&c, Path::new(&path)).unwrap();
        let count = |a: &str| rows.iter().filter(|r| r.action == a).count();
        println!(
            "filas {} | nuevos {} | omitidos {} | chile {} | extranjero {}",
            rows.len(),
            count("new"),
            count("skip"),
            rows.iter().filter(|r| r.destination == "chile").count(),
            rows.iter().filter(|r| r.destination == "foreign").count()
        );
        for r in rows
            .iter()
            .filter(|r| r.action == "skip" || r.note.is_some())
        {
            println!("fila {}: {} {:?}", r.row, r.action, r.note);
        }
    }

    #[test]
    fn order_with_several_lines_keeps_its_items() {
        let mut c = open_in_memory();
        let items = ["a", "b"].map(|n| Item {
            product_id: None,
            name: n.into(),
            quantity: 2,
            unit_price_cents: 500,
        });
        orders::create_order(
            &mut c,
            OrderInput {
                store_id: String::new(),
                external_number: "#9".into(),
                order_date: "2026-02-02".into(),
                status: None,
                shipping_cost_cents: 0,
                discount_cents: 0,
                discount_code: None,
                destination: "chile".into(),
                invoice_required: false,
                invoice_number: None,
                items: items.to_vec(),
            },
        )
        .unwrap();
        let rows = run(&c, &[("#9", "zzz", "USA")]);
        assert_eq!(rows[0].action, "update");
        apply(&mut c, rows, "pending", "2026-01-01").unwrap();
        let o = &orders::list_orders(&c, &OrderFilter::default()).unwrap()[0];
        assert_eq!(o.destination, "foreign");
        assert_eq!(orders::get_order(&c, &o.id).unwrap().items.len(), 2);
    }
}

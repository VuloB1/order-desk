//! Exportación a JSON y CSV. Lee únicamente de `business.db` y emite una lista blanca de campos
//! (structs propios, no los tipos de dominio): los datos privados no pueden colarse por construcción.
pub mod board;

use crate::db::orders;
use crate::domain::order::{OrderDetail, OrderFilter};
use crate::error::{AppError, Result};
use rusqlite::Connection;
use serde::Serialize;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Format {
    Json,
    Csv,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format> {
        match s {
            "json" => Ok(Format::Json),
            "csv" => Ok(Format::Csv),
            _ => Err(AppError::Validation(
                "Formato de exportación no válido".into(),
            )),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportItem<'a> {
    name: &'a str,
    quantity: i64,
    unit_price_cents: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportShipment<'a> {
    carrier: Option<&'a str>,
    tracking_number: Option<&'a str>,
    shipped_at: Option<&'a str>,
    delivered_at: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportHistory<'a> {
    from_status: Option<&'a str>,
    to_status: &'a str,
    changed_at: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportOrder<'a> {
    id: &'a str,
    external_number: &'a str,
    order_date: &'a str,
    status: &'a str,
    destination: &'a str,
    items: Vec<ExportItem<'a>>,
    subtotal_cents: i64,
    shipping_cost_cents: i64,
    discount_cents: i64,
    discount_code: Option<&'a str>,
    total_cents: i64,
    invoice_required: bool,
    invoice_number: Option<&'a str>,
    shipment: Option<ExportShipment<'a>>,
    history: Vec<ExportHistory<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportFile<'a> {
    version: u32,
    currency: &'static str,
    orders: Vec<ExportOrder<'a>>,
}

impl<'a> From<&'a OrderDetail> for ExportOrder<'a> {
    fn from(o: &'a OrderDetail) -> Self {
        let s = &o.summary;
        ExportOrder {
            id: &s.id,
            external_number: &s.external_number,
            order_date: &s.order_date,
            status: &s.status,
            destination: &s.destination,
            items: o
                .items
                .iter()
                .map(|i| ExportItem {
                    name: &i.name,
                    quantity: i.quantity,
                    unit_price_cents: i.unit_price_cents,
                })
                .collect(),
            subtotal_cents: s.subtotal_cents,
            shipping_cost_cents: s.shipping_cost_cents,
            discount_cents: s.discount_cents,
            discount_code: s.discount_code.as_deref(),
            total_cents: s.total_cents,
            invoice_required: s.invoice_required,
            invoice_number: s.invoice_number.as_deref(),
            shipment: o.shipment.as_ref().map(|h| ExportShipment {
                carrier: h.carrier.as_deref(),
                tracking_number: h.tracking_number.as_deref(),
                shipped_at: h.shipped_at.as_deref(),
                delivered_at: h.delivered_at.as_deref(),
            }),
            history: o
                .history
                .iter()
                .map(|h| ExportHistory {
                    from_status: h.from_status.as_deref(),
                    to_status: &h.to_status,
                    changed_at: &h.changed_at,
                })
                .collect(),
        }
    }
}

/// Pedidos que cumplen el filtro, con su detalle completo.
pub fn collect(c: &Connection, filter: &OrderFilter) -> Result<Vec<OrderDetail>> {
    orders::list_orders(c, filter)?
        .iter()
        .map(|s| orders::get_order(c, &s.id))
        .collect()
}

pub fn render(format: Format, orders: &[OrderDetail]) -> Result<Vec<u8>> {
    match format {
        Format::Json => {
            let file = ExportFile {
                version: 1,
                currency: "USD",
                orders: orders.iter().map(Into::into).collect(),
            };
            serde_json::to_vec_pretty(&file)
                .map_err(|_| AppError::Io("No se pudo generar el JSON".into()))
        }
        Format::Csv => Ok(csv(orders).into_bytes()),
    }
}

fn status_es(s: &str) -> &str {
    match s {
        "pending" => "Pendiente",
        "awaiting_chips" => "Espera chips",
        "nrf" => "Fabricación NRF",
        "classic" => "Fabricación Classic",
        "testing" => "En pruebas",
        "ready" => "Listo para enviar",
        "shipped" => "Enviado/Completado",
        "cancelled" => "Cancelado",
        other => other,
    }
}

fn destination_es(s: &str) -> &str {
    match s {
        "chile" => "Chile",
        "foreign" => "Extranjero",
        other => other,
    }
}

/// Importe en USD con coma decimal (Excel con configuración regional de Chile).
fn usd(cents: i64) -> String {
    format!("{},{:02}", cents / 100, cents % 100)
}

/// Celda de texto: se entrecomilla si hace falta y se neutraliza si Excel la tomaría por fórmula.
fn text(v: &str) -> String {
    let mut s = v.to_string();
    if s.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        s.insert(0, '\'');
    }
    if s.contains([';', '"', '\n', '\r']) {
        s = format!("\"{}\"", s.replace('"', "\"\""));
    }
    s
}

const CSV_HEADER: [&str; 16] = [
    "Nº pedido",
    "Fecha",
    "Destino",
    "Estado",
    "Productos",
    "Subtotal USD",
    "Envío USD",
    "Descuento USD",
    "Código descuento",
    "Total USD",
    "Transportista",
    "Nº seguimiento",
    "Fecha envío",
    "Fecha entrega",
    "Factura",
    "Nº factura",
];

/// CSV con BOM UTF-8 (para que Excel lea bien las tildes), separador `;` y fin de línea CRLF.
fn csv(orders: &[OrderDetail]) -> String {
    let mut out = String::from('\u{feff}');
    out.push_str(&CSV_HEADER.join(";"));
    out.push_str("\r\n");
    for o in orders {
        let s = &o.summary;
        let items = o
            .items
            .iter()
            .map(|i| format!("{}× {}", i.quantity, i.name))
            .collect::<Vec<_>>()
            .join(" | ");
        let sh = o.shipment.as_ref();
        let opt = |f: fn(&crate::domain::order::Shipment) -> &Option<String>| {
            sh.and_then(|h| f(h).as_deref())
                .map(text)
                .unwrap_or_default()
        };
        let row = [
            text(&s.external_number),
            s.order_date.clone(),
            destination_es(&s.destination).to_string(),
            status_es(&s.status).to_string(),
            text(&items),
            usd(s.subtotal_cents),
            usd(s.shipping_cost_cents),
            usd(s.discount_cents),
            s.discount_code.as_deref().map(text).unwrap_or_default(),
            usd(s.total_cents),
            opt(|h| &h.carrier),
            opt(|h| &h.tracking_number),
            opt(|h| &h.shipped_at),
            opt(|h| &h.delivered_at),
            if s.invoice_required { "Sí" } else { "No" }.to_string(),
            s.invoice_number.as_deref().map(text).unwrap_or_default(),
        ];
        out.push_str(&row.join(";"));
        out.push_str("\r\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::order::{Item, OrderInput, Shipment};

    fn seed(c: &mut Connection) {
        let store = orders::create_store(c, "Tienda Prueba").unwrap();
        let mk = |number: &str, name: &str, dest: &str| OrderInput {
            store_id: store.id.clone(),
            external_number: number.into(),
            order_date: "2026-10-01".into(),
            status: None,
            shipping_cost_cents: 1500,
            discount_cents: 250,
            discount_code: Some("PRUEBA".into()),
            destination: dest.into(),
            invoice_required: true,
            invoice_number: Some("F-1".into()),
            items: vec![Item {
                product_id: None,
                name: name.into(),
                quantity: 2,
                unit_price_cents: 1999,
            }],
        };
        let a = orders::create_order(c, mk("#1424", "Sensor; \"doble\"", "chile")).unwrap();
        orders::set_shipment(
            c,
            &a.summary.id,
            Shipment {
                carrier: Some("Correos".into()),
                tracking_number: Some("=CMD()".into()),
                shipped_at: Some("2026-10-02".into()),
                delivered_at: None,
            },
        )
        .unwrap();
        orders::create_order(c, mk("#1425", "Pulsera", "foreign")).unwrap();
    }

    #[test]
    fn json_has_only_public_fields() {
        let mut c = crate::db::open_in_memory();
        seed(&mut c);
        let all = collect(&c, &OrderFilter::default()).unwrap();
        let v: serde_json::Value =
            serde_json::from_slice(&render(Format::Json, &all).unwrap()).unwrap();
        assert_eq!(v["currency"], "USD");
        assert_eq!(v["orders"].as_array().unwrap().len(), 2);
        let mut keys: Vec<&str> = v["orders"][0]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "destination",
                "discountCents",
                "discountCode",
                "externalNumber",
                "history",
                "id",
                "invoiceNumber",
                "invoiceRequired",
                "items",
                "orderDate",
                "shipment",
                "shippingCostCents",
                "status",
                "subtotalCents",
                "totalCents",
            ]
        );
        let text = v.to_string().to_lowercase();
        for banned in ["email", "phone", "address", "customer", "buyer"] {
            assert!(!text.contains(banned), "{banned}");
        }
        assert_eq!(v["orders"][0]["totalCents"], 2 * 1999 + 1500 - 250);
    }

    #[test]
    fn csv_format_escaping_and_filter() {
        let mut c = crate::db::open_in_memory();
        seed(&mut c);
        let f = OrderFilter {
            destination: Some("chile".into()),
            ..Default::default()
        };
        let rows = collect(&c, &f).unwrap();
        assert_eq!(rows.len(), 1);
        let out = String::from_utf8(render(Format::Csv, &rows).unwrap()).unwrap();
        assert!(out.starts_with('\u{feff}'));
        let lines: Vec<&str> = out.trim_start_matches('\u{feff}').split("\r\n").collect();
        assert_eq!(lines[0].split(';').count(), 16);
        assert!(lines[1].starts_with("#1424;2026-10-01;Chile;Pendiente;"));
        assert!(
            lines[1].contains("\"2× Sensor; \"\"doble\"\"\""),
            "{}",
            lines[1]
        );
        assert!(
            lines[1].contains(";39,98;15,00;2,50;PRUEBA;52,48;Correos;'=CMD();2026-10-02;;Sí;F-1"),
            "{}",
            lines[1]
        );
    }

    #[test]
    fn money_has_two_decimals() {
        assert_eq!(usd(5), "0,05");
        assert_eq!(usd(100), "1,00");
        assert_eq!(usd(123456), "1234,56");
    }
}

use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};

pub const STATUSES: [&str; 6] = [
    "pending",
    "paid",
    "preparing",
    "shipped",
    "delivered",
    "cancelled",
];
pub const DESTINATIONS: [&str; 2] = ["chile", "foreign"];
const MAX_PRICE_CENTS: i64 = 100_000_000; // 1.000.000 USD por unidad
const MAX_QTY: i64 = 100_000;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub product_id: Option<String>,
    pub name: String,
    pub quantity: i64,
    pub unit_price_cents: i64,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OrderInput {
    /// Opcional: la aplicación trabaja con una sola tienda; vacío = la tienda por defecto (o la actual al editar).
    #[serde(default)]
    pub store_id: String,
    pub external_number: String,
    pub order_date: String,
    /// Solo se usa al crear (por defecto `pending`); los cambios posteriores van por `change_status`.
    pub status: Option<String>,
    #[serde(default)]
    pub shipping_cost_cents: i64,
    #[serde(default)]
    pub discount_cents: i64,
    /// Código aplicado; el importe ya viene calculado en `discount_cents` (copia del momento).
    pub discount_code: Option<String>,
    /// `chile` o `foreign`; si falta, `chile`.
    #[serde(default = "default_destination")]
    pub destination: String,
    /// El cliente pide factura; si hay número de factura, se considera pedida.
    #[serde(default)]
    pub invoice_required: bool,
    #[serde(default)]
    pub invoice_number: Option<String>,
    pub items: Vec<Item>,
}

fn default_destination() -> String {
    "chile".into()
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct OrderFilter {
    pub id: Option<String>,
    pub store_id: Option<String>,
    pub status: Option<String>,
    pub destination: Option<String>,
    /// Coincidencia parcial sobre el número externo.
    pub search: Option<String>,
    /// Pedidos cuyo nombre o RUT coincide con `search` (solo con la sección privada desbloqueada).
    #[serde(skip)]
    pub private_ids: Vec<String>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OrderSummary {
    pub id: String,
    pub store_id: String,
    pub store_name: String,
    pub external_number: String,
    pub status: String,
    pub order_date: String,
    pub destination: String,
    pub item_count: i64,
    pub subtotal_cents: i64,
    pub shipping_cost_cents: i64,
    pub discount_cents: i64,
    pub discount_code: Option<String>,
    pub total_cents: i64,
    pub invoice_required: bool,
    pub invoice_number: Option<String>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub from_status: Option<String>,
    pub to_status: String,
    pub changed_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Shipment {
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    pub shipped_at: Option<String>,
    pub delivered_at: Option<String>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OrderDetail {
    #[serde(flatten)]
    pub summary: OrderSummary,
    pub items: Vec<Item>,
    pub history: Vec<HistoryEntry>,
    pub shipment: Option<Shipment>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Store {
    pub id: String,
    pub name: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Product {
    pub id: String,
    pub name: String,
    pub unit_price_cents: i64,
    pub active: bool,
}

/// Ventas acumuladas de un producto (por nombre, sin contar pedidos cancelados).
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProductTotal {
    pub name: String,
    pub quantity: i64,
    pub total_cents: i64,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DiscountCode {
    pub id: String,
    pub code: String,
    /// `percent` (value = % entero 1–100) o `fixed` (value = céntimos de USD).
    pub kind: String,
    pub value: i64,
    pub active: bool,
}

/// Normaliza y valida un código de descuento.
pub fn validate_discount_code(code: &str, kind: &str, value: i64) -> Result<String> {
    let code = code.trim().to_string();
    let max = match kind {
        "percent" => 100,
        "fixed" => MAX_PRICE_CENTS,
        _ => return invalid("Tipo de descuento no válido"),
    };
    if code.is_empty() || code.chars().count() > 64 {
        return invalid("El código debe tener entre 1 y 64 caracteres");
    }
    if !(1..=max).contains(&value) {
        return invalid("Valor del descuento fuera de rango");
    }
    Ok(code)
}

fn invalid<T>(msg: &str) -> Result<T> {
    Err(AppError::Validation(msg.into()))
}

pub fn check_status(s: &str) -> Result<()> {
    if STATUSES.contains(&s) {
        Ok(())
    } else {
        invalid("Estado de pedido no válido")
    }
}

pub fn valid_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    if !b
        .iter()
        .enumerate()
        .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
    {
        return false;
    }
    let (y, m, d): (u32, u32, u32) = (
        s[0..4].parse().unwrap_or(0),
        s[5..7].parse().unwrap_or(0),
        s[8..10].parse().unwrap_or(0),
    );
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let days = match m {
        2 => 28 + leap as u32,
        4 | 6 | 9 | 11 => 30,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => return false,
    };
    y >= 1900 && (1..=days).contains(&d)
}

pub fn subtotal_cents(items: &[Item]) -> i64 {
    items.iter().fold(0i64, |a, i| {
        a.saturating_add(i.quantity.saturating_mul(i.unit_price_cents))
    })
}

pub fn validate_shipment(mut s: Shipment) -> Result<Shipment> {
    let clean = |v: Option<String>| v.map(|x| x.trim().to_string()).filter(|x| !x.is_empty());
    s.carrier = clean(s.carrier);
    s.tracking_number = clean(s.tracking_number);
    for d in [&s.shipped_at, &s.delivered_at].into_iter().flatten() {
        if !valid_date(d) {
            return invalid("Fecha de envío no válida (use AAAA-MM-DD)");
        }
    }
    // Las fechas ya son AAAA-MM-DD válidas, así que comparar el texto equivale a comparar fechas.
    if let (Some(a), Some(b)) = (&s.shipped_at, &s.delivered_at)
        && b < a
    {
        return invalid("La entrega no puede ser anterior al envío");
    }
    Ok(s)
}

/// Valida y normaliza (recorta espacios) sin alterar el formato del número externo.
pub fn validate(mut i: OrderInput) -> Result<OrderInput> {
    i.external_number = i.external_number.trim().to_string();
    if i.external_number.is_empty() || i.external_number.chars().count() > 64 {
        return invalid("El número de pedido debe tener entre 1 y 64 caracteres");
    }
    if !valid_date(&i.order_date) {
        return invalid("Fecha de pedido no válida (use AAAA-MM-DD)");
    }
    if let Some(s) = &i.status {
        check_status(s)?;
    }
    if !DESTINATIONS.contains(&i.destination.as_str()) {
        return invalid("Destino no válido");
    }
    if i.items.is_empty() {
        return invalid("El pedido necesita al menos un producto");
    }
    for it in &mut i.items {
        it.name = it.name.trim().to_string();
        if it.name.is_empty() {
            return invalid("Cada producto necesita un nombre");
        }
        if !(1..=MAX_QTY).contains(&it.quantity) {
            return invalid("Cantidad fuera de rango");
        }
        if !(0..=MAX_PRICE_CENTS).contains(&it.unit_price_cents) {
            return invalid("Precio fuera de rango");
        }
    }
    if !(0..=MAX_PRICE_CENTS).contains(&i.shipping_cost_cents) {
        return invalid("Gastos de envío fuera de rango");
    }
    if i.discount_cents < 0 || i.discount_cents > subtotal_cents(&i.items) + i.shipping_cost_cents {
        return invalid("El descuento no puede ser negativo ni superar el total");
    }
    i.discount_code = i
        .discount_code
        .take()
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty());
    if i.discount_code
        .as_ref()
        .is_some_and(|c| c.chars().count() > 64)
    {
        return invalid("El código de descuento es demasiado largo");
    }
    i.invoice_number = i
        .invoice_number
        .take()
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty());
    if i.invoice_number
        .as_ref()
        .is_some_and(|n| n.chars().count() > 64)
    {
        return invalid("El número de factura es demasiado largo");
    }
    i.invoice_required |= i.invoice_number.is_some();
    Ok(i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert!(valid_date("2026-02-28") && valid_date("2028-02-29"));
        assert!(
            !valid_date("2026-02-29")
                && !valid_date("2026-13-01")
                && !valid_date("2026-1-01")
                && !valid_date("+026-01-01")
        );
    }

    #[test]
    fn invoice_number_implies_invoice_required() {
        let mut i = OrderInput {
            store_id: String::new(),
            external_number: "#1".into(),
            order_date: "2026-03-10".into(),
            status: None,
            shipping_cost_cents: 0,
            discount_cents: 0,
            discount_code: None,
            destination: "chile".into(),
            invoice_required: false,
            invoice_number: Some("  F-123 ".into()),
            items: vec![Item {
                product_id: None,
                name: "X".into(),
                quantity: 1,
                unit_price_cents: 100,
            }],
        };
        let v = validate(i.clone()).unwrap();
        assert!(v.invoice_required);
        assert_eq!(v.invoice_number.as_deref(), Some("F-123"));
        i.invoice_number = Some("   ".into());
        let v = validate(i.clone()).unwrap();
        assert!(!v.invoice_required && v.invoice_number.is_none());
        i.invoice_number = Some("x".repeat(65));
        assert!(validate(i).is_err());
    }

    #[test]
    fn delivery_cannot_precede_shipping() {
        let ship = |a: &str, b: &str| Shipment {
            carrier: None,
            tracking_number: None,
            shipped_at: Some(a.into()),
            delivered_at: Some(b.into()),
        };
        assert!(validate_shipment(ship("2026-03-10", "2026-03-10")).is_ok());
        assert!(validate_shipment(ship("2026-03-10", "2026-03-09")).is_err());
    }

    #[test]
    fn discount_code_validation() {
        assert_eq!(
            validate_discount_code("  VERANO10 ", "percent", 10).unwrap(),
            "VERANO10"
        );
        assert_eq!(
            validate_discount_code("5USD", "fixed", 500).unwrap(),
            "5USD"
        );
        for (c, k, v) in [
            ("", "percent", 10),
            ("X", "percent", 0),
            ("X", "percent", 101),
            ("X", "fixed", 0),
            ("X", "raro", 5),
        ] {
            assert!(validate_discount_code(c, k, v).is_err(), "{c} {k} {v}");
        }
    }
}

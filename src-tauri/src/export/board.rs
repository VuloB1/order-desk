//! Tablero público de pedidos: una página HTML autocontenida (sin servidor ni peticiones externas)
//! con una columna por estado. Solo usa número, estado, destino y nombres de producto; nunca
//! datos privados ni importes. Los textos de usuario se escapan.
use crate::domain::order::OrderDetail;

/// (estado, título de columna, color en oklch). El estado «cancelled» no se muestra.
const COLUMNS: [(&str, &str, &str); 7] = [
    ("pending", "⏳ Pendientes", "oklch(0.945 0.129 101.54)"),
    (
        "awaiting_chips",
        "🧩 Espera chips",
        "oklch(0.892 0.058 10.001)",
    ),
    ("nrf", "🔧 Fabricación NRF", "oklch(0.894 0.057 293.283)"),
    (
        "classic",
        "🛠️ Fabricación Classic",
        "oklch(0.87 0.065 274.039)",
    ),
    ("testing", "🧪 En pruebas", "oklch(0.924 0.12 95.746)"),
    ("ready", "📦 Listo para enviar", "oklch(0.905 0.093 164.15)"),
    (
        "shipped",
        "🚚 Enviados/Completados",
        "oklch(0.901 0.058 230.902)",
    ),
];

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn card(o: &OrderDetail) -> String {
    let s = &o.summary;
    let kits = o
        .items
        .iter()
        .map(|i| esc(&i.name))
        .collect::<Vec<_>>()
        .join(", ");
    let (dest, class) = if s.destination == "foreign" {
        ("✈️ Extranjero", "foreign")
    } else {
        ("🚚 Chile", "chile")
    };
    format!(
        "<li class=\"card\" data-n=\"{n}\"><b>{n}</b><span class=\"kit\">{kits}</span><span class=\"dest {class}\">{dest}</span></li>",
        n = esc(&s.external_number)
    )
}

/// `generated_at`: fecha de la última publicación (AAAA-MM-DD), solo para mostrar.
pub fn render(orders: &[OrderDetail], generated_at: &str) -> String {
    let mut cols = String::new();
    for (status, title, color) in COLUMNS {
        let mut mine: Vec<&OrderDetail> = orders
            .iter()
            .filter(|o| o.summary.status == status)
            .collect();
        // Lo más reciente arriba; a igual fecha, por número.
        mine.sort_by(|a, b| {
            (&b.summary.order_date, &b.summary.external_number)
                .cmp(&(&a.summary.order_date, &a.summary.external_number))
        });
        let cards: String = mine.iter().map(|o| card(o)).collect();
        cols.push_str(&format!(
            "<section class=\"col\" style=\"--c:{color}\"><h2><span>{title}</span><em>{n}</em></h2><ul>{cards}</ul></section>",
            n = mine.len()
        ));
    }
    let date = generated_at.split('-').rev().collect::<Vec<_>>().join("/");
    include_str!("board.html")
        .replace("{{COLS}}", &cols)
        .replace("{{DATE}}", &esc(&date))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::order::{HistoryEntry, Item, OrderSummary};

    fn order(number: &str, status: &str, dest: &str, kit: &str) -> OrderDetail {
        OrderDetail {
            summary: OrderSummary {
                id: "x".into(),
                store_id: "s".into(),
                store_name: "Tienda secreta".into(),
                external_number: number.into(),
                status: status.into(),
                order_date: "2026-01-01".into(),
                destination: dest.into(),
                item_count: 1,
                subtotal_cents: 123_456,
                shipping_cost_cents: 0,
                discount_cents: 0,
                discount_code: Some("CODIGO-SECRETO".into()),
                total_cents: 123_456,
                invoice_required: true,
                invoice_number: Some("F-999".into()),
            },
            items: vec![Item {
                product_id: None,
                name: kit.into(),
                quantity: 1,
                unit_price_cents: 123_456,
            }],
            history: Vec::<HistoryEntry>::new(),
            shipment: None,
        }
    }

    #[test]
    fn board_shows_only_public_fields_and_escapes_text() {
        let html = render(
            &[
                order("#1", "nrf", "chile", "bmix6"),
                order("#2", "shipped", "foreign", "<script>alert(1)</script>"),
                order("#3", "cancelled", "chile", "oculto"),
            ],
            "2026-11-22",
        );
        assert!(html.contains("data-n=\"#1\"") && html.contains("✈️ Extranjero"));
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(!html.contains("<script>alert"));
        assert!(!html.contains("oculto"), "los cancelados no se publican");
        for secret in [
            "Tienda secreta",
            "CODIGO-SECRETO",
            "F-999",
            "123456",
            "1234,56",
        ] {
            assert!(!html.contains(secret), "{secret} no debe publicarse");
        }
        assert!(html.contains("22/11/2026"));
        for (_, title, _) in COLUMNS {
            assert!(html.contains(title));
        }
    }
}

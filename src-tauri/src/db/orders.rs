use crate::domain::order::*;
use crate::error::{AppError, Result};
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

const NOW: &str = "strftime('%Y-%m-%dT%H:%M:%fZ','now')";

fn new_id() -> String {
    Uuid::new_v4().to_string()
}

fn not_found<T>(what: &str) -> Result<T> {
    Err(AppError::NotFound(format!("{what} no encontrado")))
}

/// Sustituye el mensaje genérico de duplicado por uno concreto.
fn dup(e: rusqlite::Error, msg: String) -> AppError {
    match AppError::from(e) {
        AppError::Duplicate(_) => AppError::Duplicate(msg),
        other => other,
    }
}

pub fn create_store(c: &Connection, name: &str) -> Result<Store> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Validation(
            "El nombre de la tienda es obligatorio".into(),
        ));
    }
    let id = new_id();
    c.execute(
        "INSERT INTO stores(id, name) VALUES (?1, ?2)",
        params![id, name],
    )
    .map_err(|e| dup(e, "Ya existe una tienda con ese nombre".into()))?;
    Ok(Store {
        id,
        name: name.into(),
    })
}

/// La aplicación usa una sola tienda: la primera existente, o una "Tienda" creada al primer pedido.
fn default_store(c: &Connection) -> Result<String> {
    if let Some(id) = c
        .query_row("SELECT id FROM stores ORDER BY rowid LIMIT 1", [], |r| {
            r.get(0)
        })
        .optional()?
    {
        return Ok(id);
    }
    Ok(create_store(c, "Tienda")?.id)
}

#[cfg(test)]
pub fn list_stores(c: &Connection) -> Result<Vec<Store>> {
    let mut st = c.prepare("SELECT id, name FROM stores ORDER BY name")?;
    let rows = st.query_map([], |r| {
        Ok(Store {
            id: r.get(0)?,
            name: r.get(1)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn clean_product(name: &str, price: i64) -> Result<String> {
    let name = name.trim().to_string();
    if name.is_empty() || !(0..=100_000_000).contains(&price) {
        return Err(AppError::Validation(
            "Producto no válido (nombre vacío o precio fuera de rango)".into(),
        ));
    }
    Ok(name)
}

/// Cambiar el precio no altera pedidos existentes: cada línea guarda su propia copia.
pub fn update_product(
    c: &Connection,
    id: &str,
    name: &str,
    unit_price_cents: i64,
    active: bool,
) -> Result<Product> {
    let name = clean_product(name, unit_price_cents)?;
    let n = c.execute(
        "UPDATE products SET name=?2, unit_price_cents=?3, active=?4 WHERE id=?1",
        params![id, name, unit_price_cents, active],
    )?;
    if n == 0 {
        return not_found("Producto");
    }
    Ok(Product {
        id: id.into(),
        name,
        unit_price_cents,
        active,
    })
}

pub fn create_product(c: &Connection, name: &str, unit_price_cents: i64) -> Result<Product> {
    let name = clean_product(name, unit_price_cents)?;
    let id = new_id();
    c.execute(
        "INSERT INTO products(id, name, unit_price_cents, sort_order)
         VALUES (?1, ?2, ?3, (SELECT COALESCE(MAX(sort_order), -1) + 1 FROM products))",
        params![id, name, unit_price_cents],
    )?;
    Ok(Product {
        id,
        name,
        unit_price_cents,
        active: true,
    })
}

/// Borra el producto; los pedidos conservan su copia (nombre y precio) y solo pierden el enlace.
pub fn delete_product(c: &Connection, id: &str) -> Result<()> {
    if c.execute("DELETE FROM products WHERE id = ?1", [id])? == 0 {
        return not_found("Producto");
    }
    Ok(())
}

pub fn list_products(c: &Connection) -> Result<Vec<Product>> {
    let mut st = c.prepare(
        "SELECT id, name, unit_price_cents, active FROM products ORDER BY sort_order, name",
    )?;
    let rows = st.query_map([], |r| {
        Ok(Product {
            id: r.get(0)?,
            name: r.get(1)?,
            unit_price_cents: r.get(2)?,
            active: r.get(3)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Fija el orden de los productos: `ids` debe contener exactamente todos los productos, una vez.
pub fn reorder_products(c: &mut Connection, ids: &[String]) -> Result<()> {
    let tx = c.transaction()?;
    let mut existing: Vec<String> = tx
        .prepare("SELECT id FROM products")?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let mut given = ids.to_vec();
    existing.sort();
    given.sort();
    if existing != given {
        return Err(AppError::Validation(
            "La lista de productos no coincide con el catálogo".into(),
        ));
    }
    for (pos, id) in ids.iter().enumerate() {
        tx.execute(
            "UPDATE products SET sort_order = ?2 WHERE id = ?1",
            params![id, pos as i64],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn create_discount_code(
    c: &Connection,
    code: &str,
    kind: &str,
    value: i64,
) -> Result<DiscountCode> {
    let code = validate_discount_code(code, kind, value)?;
    let id = new_id();
    c.execute(
        "INSERT INTO discount_codes(id, code, kind, value) VALUES (?1, ?2, ?3, ?4)",
        params![id, code, kind, value],
    )
    .map_err(|e| dup(e, "Ya existe un código con ese nombre".into()))?;
    Ok(DiscountCode {
        id,
        code,
        kind: kind.into(),
        value,
        active: true,
    })
}

/// Cambiar o desactivar un código no altera pedidos ya creados (guardan su importe).
pub fn update_discount_code(
    c: &Connection,
    id: &str,
    code: &str,
    kind: &str,
    value: i64,
    active: bool,
) -> Result<DiscountCode> {
    let code = validate_discount_code(code, kind, value)?;
    let n = c
        .execute(
            "UPDATE discount_codes SET code=?2, kind=?3, value=?4, active=?5 WHERE id=?1",
            params![id, code, kind, value, active],
        )
        .map_err(|e| dup(e, "Ya existe un código con ese nombre".into()))?;
    if n == 0 {
        return not_found("Código");
    }
    Ok(DiscountCode {
        id: id.into(),
        code,
        kind: kind.into(),
        value,
        active,
    })
}

pub fn delete_discount_code(c: &Connection, id: &str) -> Result<()> {
    if c.execute("DELETE FROM discount_codes WHERE id = ?1", [id])? == 0 {
        return not_found("Código de descuento");
    }
    Ok(())
}

pub fn list_discount_codes(c: &Connection) -> Result<Vec<DiscountCode>> {
    let mut st =
        c.prepare("SELECT id, code, kind, value, active FROM discount_codes ORDER BY code")?;
    let rows = st.query_map([], |r| {
        Ok(DiscountCode {
            id: r.get(0)?,
            code: r.get(1)?,
            kind: r.get(2)?,
            value: r.get(3)?,
            active: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn insert_items(c: &Connection, order_id: &str, items: &[Item]) -> Result<()> {
    let mut st = c.prepare(
        "INSERT INTO order_items(order_id, product_id, name, quantity, unit_price_cents) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for it in items {
        st.execute(params![
            order_id,
            it.product_id,
            it.name,
            it.quantity,
            it.unit_price_cents
        ])?;
    }
    Ok(())
}

pub fn create_order(c: &mut Connection, input: OrderInput) -> Result<OrderDetail> {
    let i = validate(input)?;
    let status = i.status.clone().unwrap_or_else(|| "pending".into());
    let id = new_id();
    let tx = c.transaction()?;
    let store_id = if i.store_id.is_empty() {
        default_store(&tx)?
    } else {
        i.store_id.clone()
    };
    tx.execute(
        "INSERT INTO orders(id, store_id, external_number, status, order_date, shipping_cost_cents, discount_cents, discount_code, destination, invoice_required, invoice_number)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![id, store_id, i.external_number, status, i.order_date, i.shipping_cost_cents, i.discount_cents, i.discount_code, i.destination, i.invoice_required, i.invoice_number],
    )
    .map_err(|e| dup(e, format!("El pedido {} ya existe en esa tienda", i.external_number)))?;
    insert_items(&tx, &id, &i.items)?;
    tx.execute(
        "INSERT INTO status_history(order_id, from_status, to_status) VALUES (?1, NULL, ?2)",
        params![id, status],
    )?;
    tx.commit()?;
    get_order(c, &id)
}

pub fn update_order(c: &mut Connection, id: &str, input: OrderInput) -> Result<OrderDetail> {
    let i = validate(input)?;
    let tx = c.transaction()?;
    let n = tx
        .execute(
            &format!(
                "UPDATE orders SET store_id=COALESCE(NULLIF(?2, ''), store_id), external_number=?3, order_date=?4, shipping_cost_cents=?5,
                 discount_cents=?6, discount_code=?7, destination=?8, invoice_required=?9, invoice_number=?10, updated_at={NOW} WHERE id=?1"
            ),
            params![id, i.store_id, i.external_number, i.order_date, i.shipping_cost_cents, i.discount_cents, i.discount_code, i.destination, i.invoice_required, i.invoice_number],
        )
        .map_err(|e| dup(e, format!("El pedido {} ya existe en esa tienda", i.external_number)))?;
    if n == 0 {
        return not_found("Pedido");
    }
    tx.execute("DELETE FROM order_items WHERE order_id = ?1", [id])?;
    insert_items(&tx, id, &i.items)?;
    tx.commit()?;
    get_order(c, id)
}

pub fn change_status(c: &mut Connection, id: &str, new_status: &str) -> Result<OrderDetail> {
    check_status(new_status)?;
    let tx = c.transaction()?;
    let Some(current) = tx
        .query_row("SELECT status FROM orders WHERE id = ?1", [id], |r| {
            r.get::<_, String>(0)
        })
        .optional()?
    else {
        return not_found("Pedido");
    };
    if current == new_status {
        return Err(AppError::Validation(
            "El pedido ya está en ese estado".into(),
        ));
    }
    tx.execute(
        &format!("UPDATE orders SET status = ?2, updated_at = {NOW} WHERE id = ?1"),
        params![id, new_status],
    )?;
    tx.execute(
        "INSERT INTO status_history(order_id, from_status, to_status) VALUES (?1, ?2, ?3)",
        params![id, current, new_status],
    )?;
    tx.commit()?;
    get_order(c, id)
}

pub fn set_shipment(c: &Connection, id: &str, shipment: Shipment) -> Result<OrderDetail> {
    let s = validate_shipment(shipment)?;
    if c.query_row("SELECT 1 FROM orders WHERE id = ?1", [id], |_| Ok(()))
        .optional()?
        .is_none()
    {
        return not_found("Pedido");
    }
    c.execute(
        "INSERT INTO shipments(order_id, carrier, tracking_number, shipped_at, delivered_at) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(order_id) DO UPDATE SET carrier=excluded.carrier, tracking_number=excluded.tracking_number,
         shipped_at=excluded.shipped_at, delivered_at=excluded.delivered_at",
        params![id, s.carrier, s.tracking_number, s.shipped_at, s.delivered_at],
    )?;
    get_order(c, id)
}

/// Número del último pedido añadido (el más reciente por fecha de creación, no el de número mayor).
pub fn last_order_number(c: &Connection) -> Result<Option<String>> {
    Ok(c.query_row(
        "SELECT external_number FROM orders ORDER BY created_at DESC, rowid DESC LIMIT 1",
        [],
        |r| r.get(0),
    )
    .optional()?)
}

/// Borra el pedido y, por cascada, sus líneas, historial y envío. Su fila en `private.db` la borra el comando.
pub fn delete_order(c: &Connection, id: &str) -> Result<()> {
    match c.execute("DELETE FROM orders WHERE id = ?1", [id])? {
        0 => not_found("Pedido"),
        _ => Ok(()),
    }
}

/// Patrón para `LIKE ... ESCAPE '\\'`: los comodines del texto buscado se toman literales.
pub fn like_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Los productos más vendidos por unidades (los pedidos cancelados no cuentan).
pub fn top_products(c: &Connection, limit: i64) -> Result<Vec<ProductTotal>> {
    let mut st = c.prepare(
        "SELECT i.name, SUM(i.quantity), SUM(i.quantity * i.unit_price_cents)
         FROM order_items i JOIN orders o ON o.id = i.order_id
         WHERE o.status <> 'cancelled'
         GROUP BY i.name ORDER BY SUM(i.quantity) DESC, i.name LIMIT ?1",
    )?;
    let rows = st.query_map([limit.clamp(1, 50)], |r| {
        Ok(ProductTotal {
            name: r.get(0)?,
            quantity: r.get(1)?,
            total_cents: r.get(2)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn list_orders(c: &Connection, f: &OrderFilter) -> Result<Vec<OrderSummary>> {
    let search = f
        .search
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(like_escape);
    let mut st = c.prepare(
        "SELECT o.id, o.store_id, s.name, o.external_number, o.status, o.order_date,
                o.shipping_cost_cents, o.discount_cents, o.discount_code,
                COALESCE((SELECT SUM(quantity * unit_price_cents) FROM order_items WHERE order_id = o.id), 0),
                COALESCE((SELECT SUM(quantity) FROM order_items WHERE order_id = o.id), 0),
                o.destination, o.invoice_required, o.invoice_number
         FROM orders o JOIN stores s ON s.id = o.store_id
         WHERE (?1 IS NULL OR o.id = ?1) AND (?2 IS NULL OR o.store_id = ?2) AND (?3 IS NULL OR o.status = ?3)
           AND (?4 IS NULL OR o.external_number LIKE '%' || ?4 || '%' ESCAPE '\\'
                OR o.id IN (SELECT value FROM json_each(?6)))
           AND (?5 IS NULL OR o.destination = ?5)
         ORDER BY o.order_date DESC, o.created_at DESC, o.rowid DESC",
    )?;
    let rows = st.query_map(
        params![
            f.id,
            f.store_id,
            f.status,
            search,
            f.destination,
            serde_json::to_string(&f.private_ids).unwrap_or_else(|_| "[]".into())
        ],
        |r| {
            let (shipping, discount, sub): (i64, i64, i64) = (r.get(6)?, r.get(7)?, r.get(9)?);
            let total = sub + shipping - discount;
            Ok(OrderSummary {
                id: r.get(0)?,
                store_id: r.get(1)?,
                store_name: r.get(2)?,
                external_number: r.get(3)?,
                status: r.get(4)?,
                order_date: r.get(5)?,
                destination: r.get(11)?,
                item_count: r.get(10)?,
                subtotal_cents: sub,
                shipping_cost_cents: shipping,
                discount_cents: discount,
                discount_code: r.get(8)?,
                total_cents: total,
                invoice_required: r.get(12)?,
                invoice_number: r.get(13)?,
            })
        },
    )?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn get_order(c: &Connection, id: &str) -> Result<OrderDetail> {
    let filter = OrderFilter {
        id: Some(id.into()),
        ..Default::default()
    };
    let Some(summary) = list_orders(c, &filter)?.into_iter().next() else {
        return not_found("Pedido");
    };
    let items = c
        .prepare("SELECT product_id, name, quantity, unit_price_cents FROM order_items WHERE order_id = ?1 ORDER BY id")?
        .query_map([id], |r| {
            Ok(Item { product_id: r.get(0)?, name: r.get(1)?, quantity: r.get(2)?, unit_price_cents: r.get(3)? })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let history = c
        .prepare("SELECT from_status, to_status, changed_at FROM status_history WHERE order_id = ?1 ORDER BY id")?
        .query_map([id], |r| Ok(HistoryEntry { from_status: r.get(0)?, to_status: r.get(1)?, changed_at: r.get(2)? }))?
        .collect::<rusqlite::Result<_>>()?;
    let shipment = c
        .query_row("SELECT carrier, tracking_number, shipped_at, delivered_at FROM shipments WHERE order_id = ?1", [id], |r| {
            Ok(Shipment { carrier: r.get(0)?, tracking_number: r.get(1)?, shipped_at: r.get(2)?, delivered_at: r.get(3)? })
        })
        .optional()?;
    Ok(OrderDetail {
        summary,
        items,
        history,
        shipment,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{migrate, open_in_memory};

    fn input(store: &str, number: &str) -> OrderInput {
        OrderInput {
            store_id: store.into(),
            external_number: number.into(),
            order_date: "2026-03-10".into(),
            status: None,
            shipping_cost_cents: 500,
            discount_cents: 0,
            discount_code: None,
            destination: "chile".into(),
            invoice_required: false,
            invoice_number: None,
            items: vec![
                Item {
                    product_id: None,
                    name: "Tracker de prueba".into(),
                    quantity: 2,
                    unit_price_cents: 2_500,
                },
                Item {
                    product_id: None,
                    name: "Extensión de prueba".into(),
                    quantity: 1,
                    unit_price_cents: 1_000,
                },
            ],
        }
    }

    fn setup() -> (Connection, String) {
        let c = open_in_memory();
        let store = create_store(&c, "Tienda A").unwrap().id;
        (c, store)
    }

    #[test]
    fn migrations_set_version_and_are_idempotent() {
        let mut c = open_in_memory();
        let v = |c: &Connection| {
            c.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                .unwrap()
        };
        assert_eq!(v(&c), 5);
        migrate(&mut c).unwrap();
        assert_eq!(v(&c), 5);
        c.pragma_update(None, "user_version", 99).unwrap();
        assert!(migrate(&mut c).is_err());
    }

    #[test]
    fn totals_in_usd() {
        let (mut c, s) = setup();
        let o = create_order(&mut c, input(&s, "#1424")).unwrap();
        assert_eq!(o.summary.subtotal_cents, 6_000);
        assert_eq!(o.summary.total_cents, 6_500); // 65,00 USD
        assert_eq!(o.summary.item_count, 3);
        assert_eq!(o.summary.external_number, "#1424");
        assert_eq!(o.history.len(), 1);
        assert_eq!(o.history[0].to_status, "pending");
    }

    #[test]
    fn discount_is_stored_with_its_code() {
        let (mut c, s) = setup();
        let mut i = input(&s, "#1");
        i.discount_cents = 600;
        i.discount_code = Some("  VERANO10 ".into());
        let o = create_order(&mut c, i).unwrap();
        assert_eq!(
            (o.summary.discount_cents, o.summary.total_cents),
            (600, 5_900)
        );
        assert_eq!(o.summary.discount_code.as_deref(), Some("VERANO10"));
        let mut j = input(&s, "#2");
        j.discount_code = Some("   ".into());
        assert_eq!(create_order(&mut c, j).unwrap().summary.discount_code, None);
    }

    #[test]
    fn discount_code_catalog() {
        let (mut c, s) = setup();
        let a = create_discount_code(&c, "VERANO10", "percent", 10).unwrap();
        create_discount_code(&c, "5USD", "fixed", 500).unwrap();
        assert!(matches!(
            create_discount_code(&c, "verano10", "percent", 20),
            Err(AppError::Duplicate(_))
        )); // sin distinguir mayúsculas
        assert!(create_discount_code(&c, "X", "percent", 101).is_err());
        assert!(create_discount_code(&c, "", "fixed", 5).is_err());
        assert_eq!(list_discount_codes(&c).unwrap().len(), 2);

        let mut i = input(&s, "#1");
        i.discount_cents = 600;
        i.discount_code = Some(a.code.clone());
        let id = create_order(&mut c, i).unwrap().summary.id;
        let u = update_discount_code(&c, &a.id, "VERANO10", "percent", 50, false).unwrap();
        assert!(!u.active);
        assert_eq!(get_order(&c, &id).unwrap().summary.discount_cents, 600); // el pedido no cambia
        assert!(matches!(
            update_discount_code(&c, "no-existe", "X", "fixed", 1, true),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn products_keep_manual_order() {
        let mut c = open_in_memory();
        let ids: Vec<String> = ["Zeta de prueba", "Alfa de prueba", "Medio de prueba"]
            .iter()
            .map(|n| create_product(&c, n, 100).unwrap().id)
            .collect();
        let names = |c: &Connection| -> Vec<String> {
            list_products(c)
                .unwrap()
                .into_iter()
                .map(|p| p.name)
                .collect()
        };
        // Orden de creación, no alfabético.
        assert_eq!(
            names(&c),
            ["Zeta de prueba", "Alfa de prueba", "Medio de prueba"]
        );
        reorder_products(&mut c, &[ids[2].clone(), ids[0].clone(), ids[1].clone()]).unwrap();
        assert_eq!(
            names(&c),
            ["Medio de prueba", "Zeta de prueba", "Alfa de prueba"]
        );
        let extra = create_product(&c, "Nuevo de prueba", 1).unwrap(); // va al final
        assert_eq!(names(&c).last().unwrap(), "Nuevo de prueba");
        // Listas incompletas, repetidas o ajenas se rechazan sin cambiar nada.
        for bad in [
            vec![ids[0].clone()],
            vec![ids[0].clone(); 4],
            vec!["x".to_string(); 4],
        ] {
            assert!(reorder_products(&mut c, &bad).is_err());
        }
        assert_eq!(names(&c).last().unwrap(), &extra.name);
    }

    #[test]
    fn migration_from_v1_keeps_data() {
        let mut c = Connection::open_in_memory().unwrap();
        c.pragma_update(None, "foreign_keys", true).unwrap();
        c.execute_batch(include_str!("migrations/0001_init.sql"))
            .unwrap();
        c.pragma_update(None, "user_version", 1).unwrap();
        c.execute_batch(
            "INSERT INTO stores VALUES ('s1','Tienda A');
             INSERT INTO products(id,name) VALUES ('p1','Zeta'),('p2','alfa'),('p3','Beta');
             INSERT INTO orders(id,store_id,external_number,status,order_date,clp_per_usd)
                 VALUES ('o1','s1','#1','paid','2026-01-01',950.5);
             INSERT INTO order_items(order_id,name,quantity,unit_price_cents) VALUES ('o1','Algo',2,500);",
        )
        .unwrap();
        migrate(&mut c).unwrap();
        let o = get_order(&c, "o1").unwrap();
        assert_eq!(
            (o.summary.total_cents, o.summary.status.as_str()),
            (1_000, "paid")
        );
        assert_eq!(o.summary.discount_code, None);
        assert_eq!(o.summary.destination, "chile"); // los pedidos antiguos se consideran de Chile
        // El orden alfabético previo (sin distinguir mayúsculas) se conserva.
        let names: Vec<_> = list_products(&c)
            .unwrap()
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(names, ["alfa", "Beta", "Zeta"]);
        let cols: Vec<String> = c
            .prepare("SELECT name FROM pragma_table_info('orders')")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert!(!cols.contains(&"clp_per_usd".to_string()));
    }

    #[test]
    fn external_number_unique_per_store_only() {
        let (mut c, s) = setup();
        let s2 = create_store(&c, "Tienda B").unwrap().id;
        create_order(&mut c, input(&s, "#1424")).unwrap();
        assert!(matches!(
            create_order(&mut c, input(&s, "#1424")),
            Err(AppError::Duplicate(_))
        ));
        assert!(matches!(
            create_order(&mut c, input(&s, "  #1424 ")),
            Err(AppError::Duplicate(_))
        )); // se recorta
        create_order(&mut c, input(&s2, "#1424")).unwrap(); // otra tienda: permitido
        create_order(&mut c, input(&s, "#01424")).unwrap(); // formato distinto: otro número
    }

    #[test]
    fn rejects_invalid_input() {
        let (mut c, s) = setup();
        let bad: Vec<Box<dyn Fn(&mut OrderInput)>> = vec![
            Box::new(|i| i.external_number = "   ".into()),
            Box::new(|i| i.order_date = "2026-02-30".into()),
            Box::new(|i| i.items.clear()),
            Box::new(|i| i.items[0].quantity = 0),
            Box::new(|i| i.items[0].unit_price_cents = -1),
            Box::new(|i| i.discount_cents = 999_999),
            Box::new(|i| i.discount_cents = -1),
            Box::new(|i| i.destination = "marte".into()),
            Box::new(|i| i.status = Some("inventado".into())),
        ];
        for f in bad {
            let mut i = input(&s, "#9");
            f(&mut i);
            assert!(matches!(
                create_order(&mut c, i),
                Err(AppError::Validation(_))
            ));
        }
        assert!(matches!(
            create_order(&mut c, input("no-existe", "#9")),
            Err(AppError::NotFound(_))
        ));
        assert!(list_orders(&c, &OrderFilter::default()).unwrap().is_empty()); // nada quedó a medias
    }

    #[test]
    fn status_changes_are_recorded() {
        let (mut c, s) = setup();
        let id = create_order(&mut c, input(&s, "#1")).unwrap().summary.id;
        let o = change_status(&mut c, &id, "paid").unwrap();
        assert_eq!(o.summary.status, "paid");
        let o = change_status(&mut c, &id, "shipped").unwrap();
        let h: Vec<_> = o
            .history
            .iter()
            .map(|h| (h.from_status.as_deref(), h.to_status.as_str()))
            .collect();
        assert_eq!(
            h,
            [
                (None, "pending"),
                (Some("pending"), "paid"),
                (Some("paid"), "shipped")
            ]
        );
        assert!(change_status(&mut c, &id, "shipped").is_err());
        assert!(change_status(&mut c, &id, "xx").is_err());
        assert!(matches!(
            change_status(&mut c, "no-existe", "paid"),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn update_replaces_items_and_rolls_back_on_conflict() {
        let (mut c, s) = setup();
        let a = create_order(&mut c, input(&s, "#1")).unwrap().summary.id;
        create_order(&mut c, input(&s, "#2")).unwrap();
        let mut i = input(&s, "#1");
        i.items = vec![Item {
            product_id: None,
            name: "Solo uno".into(),
            quantity: 1,
            unit_price_cents: 100,
        }];
        let o = update_order(&mut c, &a, i).unwrap();
        assert_eq!(o.items.len(), 1);
        assert_eq!(o.summary.subtotal_cents, 100);

        let mut conflict = input(&s, "#2"); // choca con el otro pedido
        conflict.items[0].name = "No debe guardarse".into();
        assert!(matches!(
            update_order(&mut c, &a, conflict),
            Err(AppError::Duplicate(_))
        ));
        let o = get_order(&c, &a).unwrap();
        assert_eq!(
            (o.summary.external_number.as_str(), o.items[0].name.as_str()),
            ("#1", "Solo uno")
        );
        assert!(matches!(
            update_order(&mut c, "no-existe", input(&s, "#3")),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn shipment_upsert_and_delete_cascade() {
        let (mut c, s) = setup();
        let id = create_order(&mut c, input(&s, "#1")).unwrap().summary.id;
        let sh = Shipment {
            carrier: Some(" Correos ".into()),
            tracking_number: Some("TEST123".into()),
            shipped_at: Some("2026-03-11".into()),
            delivered_at: None,
        };
        let o = set_shipment(&c, &id, sh).unwrap();
        assert_eq!(o.shipment.unwrap().carrier.as_deref(), Some("Correos"));
        let sh2 = Shipment {
            carrier: None,
            tracking_number: Some("TEST456".into()),
            shipped_at: None,
            delivered_at: Some("2026-03-15".into()),
        };
        assert_eq!(
            set_shipment(&c, &id, sh2)
                .unwrap()
                .shipment
                .unwrap()
                .tracking_number
                .as_deref(),
            Some("TEST456")
        );
        let bad = Shipment {
            carrier: None,
            tracking_number: None,
            shipped_at: Some("ayer".into()),
            delivered_at: None,
        };
        assert!(set_shipment(&c, &id, bad).is_err());

        delete_order(&c, &id).unwrap();
        for t in ["order_items", "status_history", "shipments"] {
            assert_eq!(
                c.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r
                    .get::<_, i64>(0))
                    .unwrap(),
                0,
                "{t}"
            );
        }
        assert!(matches!(delete_order(&c, &id), Err(AppError::NotFound(_))));
    }

    #[test]
    fn destination_is_stored_edited_and_filtered() {
        let (mut c, s) = setup();
        let a = create_order(&mut c, input(&s, "#1")).unwrap();
        assert_eq!(a.summary.destination, "chile");
        let mut f = input(&s, "#2");
        f.destination = "foreign".into();
        let b = create_order(&mut c, f).unwrap().summary.id;
        fn by(c: &Connection, d: &str) -> usize {
            let filter = OrderFilter {
                destination: Some(d.into()),
                ..Default::default()
            };
            list_orders(c, &filter).unwrap().len()
        }
        assert_eq!((by(&c, "chile"), by(&c, "foreign")), (1, 1));
        let mut e = input(&s, "#2");
        e.destination = "chile".into();
        assert_eq!(
            update_order(&mut c, &b, e).unwrap().summary.destination,
            "chile"
        );
        assert_eq!(by(&c, "chile"), 2);
    }

    #[test]
    fn filters_and_search() {
        let (mut c, s) = setup();
        let s2 = create_store(&c, "Tienda B").unwrap().id;
        let a = create_order(&mut c, input(&s, "#1424")).unwrap().summary.id;
        create_order(&mut c, input(&s2, "#1500")).unwrap();
        create_order(&mut c, input(&s, "A_%1")).unwrap();
        change_status(&mut c, &a, "paid").unwrap();
        let n = |f: OrderFilter| list_orders(&c, &f).unwrap().len();
        assert_eq!(n(OrderFilter::default()), 3);
        assert_eq!(
            n(OrderFilter {
                store_id: Some(s2.clone()),
                ..Default::default()
            }),
            1
        );
        assert_eq!(
            n(OrderFilter {
                status: Some("paid".into()),
                ..Default::default()
            }),
            1
        );
        assert_eq!(
            n(OrderFilter {
                search: Some("14".into()),
                ..Default::default()
            }),
            1
        );
        assert_eq!(
            n(OrderFilter {
                search: Some("_%".into()),
                ..Default::default()
            }),
            1
        ); // comodines escapados
    }

    #[test]
    fn product_catalog() {
        let c = open_in_memory();
        let p = create_product(&c, "Producto de prueba", 9_900).unwrap();
        assert!(create_product(&c, "", 1).is_err());
        assert_eq!(list_products(&c).unwrap().len(), 1);
        delete_product(&c, &p.id).unwrap();
        assert!(list_products(&c).unwrap().is_empty());
        assert!(matches!(
            delete_product(&c, &p.id),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn product_update_keeps_existing_order_prices() {
        let (mut c, s) = setup();
        let p = create_product(&c, "Producto de prueba", 9_900).unwrap();
        let mut i = input(&s, "#1");
        i.items = vec![Item {
            product_id: Some(p.id.clone()),
            name: p.name.clone(),
            quantity: 3,
            unit_price_cents: p.unit_price_cents,
        }];
        let id = create_order(&mut c, i).unwrap().summary.id;

        let u = update_product(&c, &p.id, "Renombrado", 12_000, false).unwrap();
        assert!(!u.active);
        assert!(
            !list_products(&c)
                .unwrap()
                .iter()
                .find(|x| x.id == p.id)
                .unwrap()
                .active
        );
        let o = get_order(&c, &id).unwrap();
        assert_eq!(
            (o.items[0].unit_price_cents, o.items[0].name.as_str()),
            (9_900, "Producto de prueba")
        );

        assert!(matches!(
            update_product(&c, "no-existe", "X", 1, true),
            Err(AppError::NotFound(_))
        ));
        assert!(update_product(&c, &p.id, "", 1, true).is_err());
    }

    #[test]
    fn top_products_ignore_cancelled_orders() {
        let mut c = open_in_memory();
        create_order(&mut c, input("", "#1")).unwrap(); // 2 × "Tracker de prueba" + otro producto
        let b = create_order(&mut c, input("", "#2")).unwrap();
        change_status(&mut c, &b.summary.id, "cancelled").unwrap();
        let top = top_products(&c, 5).unwrap();
        assert_eq!(top[0].name, "Tracker de prueba");
        assert_eq!(top[0].quantity, 2); // solo el pedido no cancelado
        assert_eq!(top[0].total_cents, 2 * 2_500);
        assert_eq!(top_products(&c, 1).unwrap().len(), 1);
    }

    #[test]
    fn private_ids_extend_the_search() {
        let mut c = open_in_memory();
        let a = create_order(&mut c, input("", "#1")).unwrap();
        create_order(&mut c, input("", "#2")).unwrap();
        let find = |search: &str, ids: Vec<String>| {
            let f = OrderFilter {
                search: Some(search.into()),
                private_ids: ids,
                ..Default::default()
            };
            list_orders(&c, &f).unwrap().len()
        };
        assert_eq!(find("Persona", vec![]), 0);
        assert_eq!(find("Persona", vec![a.summary.id.clone()]), 1);
        assert_eq!(find("#2", vec![a.summary.id]), 2); // número o coincidencia privada
    }

    #[test]
    fn invoice_is_stored_edited_and_listed() {
        let mut c = open_in_memory();
        let o = create_order(&mut c, input("", "#1")).unwrap();
        assert!(!o.summary.invoice_required && o.summary.invoice_number.is_none());
        let mut e = input("", "#1");
        e.invoice_required = true;
        let u = update_order(&mut c, &o.summary.id, e.clone()).unwrap();
        assert!(u.summary.invoice_required && u.summary.invoice_number.is_none());
        e.invoice_number = Some("F-77".into());
        e.invoice_required = false; // el número implica factura
        let u = update_order(&mut c, &o.summary.id, e).unwrap();
        assert!(u.summary.invoice_required);
        let l = list_orders(&c, &OrderFilter::default()).unwrap();
        assert_eq!(l[0].invoice_number.as_deref(), Some("F-77"));
    }

    #[test]
    fn single_store_is_used_when_none_is_given() {
        let mut c = open_in_memory();
        let mut a = input("", "#1");
        a.store_id = String::new();
        let o1 = create_order(&mut c, a.clone()).unwrap();
        let mut b = a.clone();
        b.external_number = "#2".into();
        let o2 = create_order(&mut c, b).unwrap();
        assert_eq!(o1.summary.store_id, o2.summary.store_id);
        assert_eq!(list_stores(&c).unwrap().len(), 1);
        // Editar sin tienda conserva la actual.
        let mut e = a;
        e.external_number = "#1b".into();
        let u = update_order(&mut c, &o1.summary.id, e).unwrap();
        assert_eq!(u.summary.store_id, o1.summary.store_id);
        // El número sigue siendo único en la tienda única.
        let mut d = input("", "#2");
        d.store_id = String::new();
        assert!(matches!(
            create_order(&mut c, d),
            Err(AppError::Duplicate(_))
        ));
    }

    #[test]
    fn discount_codes_can_be_deleted_without_touching_orders() {
        let (mut c, s) = setup();
        let code = create_discount_code(&c, "PRUEBA", "percent", 10).unwrap();
        let mut i = input(&s, "#1");
        i.discount_code = Some("PRUEBA".into());
        i.discount_cents = 100;
        let id = create_order(&mut c, i).unwrap().summary.id;
        delete_discount_code(&c, &code.id).unwrap();
        assert!(list_discount_codes(&c).unwrap().is_empty());
        assert_eq!(
            get_order(&c, &id).unwrap().summary.discount_code.as_deref(),
            Some("PRUEBA")
        );
        assert!(matches!(
            delete_discount_code(&c, &code.id),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn deleting_a_product_keeps_order_lines() {
        let (mut c, s) = setup();
        let p = create_product(&c, "Sensor", 500).unwrap();
        let mut i = input(&s, "#1");
        i.items = vec![Item {
            product_id: Some(p.id.clone()),
            name: p.name.clone(),
            quantity: 2,
            unit_price_cents: 500,
        }];
        let id = create_order(&mut c, i).unwrap().summary.id;
        delete_product(&c, &p.id).unwrap();
        let o = get_order(&c, &id).unwrap();
        assert_eq!(
            (o.items[0].name.as_str(), o.items[0].product_id.clone()),
            ("Sensor", None)
        );
        assert_eq!(o.summary.total_cents, 1_500); // 2 × 500 + 500 de envío
    }

    #[test]
    fn last_order_number_is_the_most_recently_added() {
        let (mut c, s) = setup();
        assert_eq!(last_order_number(&c).unwrap(), None);
        create_order(&mut c, input(&s, "#1313")).unwrap();
        create_order(&mut c, input(&s, "#1200")).unwrap(); // más bajo, pero añadido después
        assert_eq!(last_order_number(&c).unwrap().as_deref(), Some("#1200"));
    }
}

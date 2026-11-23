-- Nuevos estados de pedido (flujo de fabricación). SQLite no permite cambiar un CHECK: se reconstruye
-- la tabla `orders` (la migración corre con foreign_keys=OFF, ver `migrate_with`).
-- Equivalencias: pagado y preparando -> pendiente; entregado -> enviado/completado.
CREATE TABLE orders_new (
    id                  TEXT PRIMARY KEY,
    store_id            TEXT NOT NULL REFERENCES stores(id),
    external_number     TEXT NOT NULL CHECK (length(trim(external_number)) > 0),
    status              TEXT NOT NULL CHECK (status IN ('pending','awaiting_chips','nrf','classic','testing','ready','shipped','cancelled')),
    order_date          TEXT NOT NULL,
    shipping_cost_cents INTEGER NOT NULL DEFAULT 0 CHECK (shipping_cost_cents >= 0),
    discount_cents      INTEGER NOT NULL DEFAULT 0 CHECK (discount_cents >= 0),
    created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    discount_code       TEXT,
    destination         TEXT NOT NULL DEFAULT 'chile' CHECK (destination IN ('chile', 'foreign')),
    invoice_required    INTEGER NOT NULL DEFAULT 0,
    invoice_number      TEXT,
    UNIQUE (store_id, external_number)
);

INSERT INTO orders_new (id, store_id, external_number, status, order_date, shipping_cost_cents, discount_cents,
                        created_at, updated_at, discount_code, destination, invoice_required, invoice_number)
SELECT id, store_id, external_number,
       CASE status WHEN 'paid' THEN 'pending' WHEN 'preparing' THEN 'pending' WHEN 'delivered' THEN 'shipped' ELSE status END,
       order_date, shipping_cost_cents, discount_cents, created_at, updated_at, discount_code, destination,
       invoice_required, invoice_number
FROM orders;

DROP TABLE orders;
ALTER TABLE orders_new RENAME TO orders;
CREATE INDEX idx_orders_status ON orders(status);
CREATE INDEX idx_orders_date   ON orders(order_date);

UPDATE status_history SET
    from_status = CASE from_status WHEN 'paid' THEN 'pending' WHEN 'preparing' THEN 'pending' WHEN 'delivered' THEN 'shipped' ELSE from_status END,
    to_status   = CASE to_status   WHEN 'paid' THEN 'pending' WHEN 'preparing' THEN 'pending' WHEN 'delivered' THEN 'shipped' ELSE to_status END;
-- Tras la equivalencia, un cambio de un estado a sí mismo ya no aporta nada.
DELETE FROM status_history WHERE from_status = to_status;

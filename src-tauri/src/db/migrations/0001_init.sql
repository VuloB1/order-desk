-- Importes en céntimos de USD. El valor en CLP se deriva con orders.clp_per_usd.
-- Sin campos de texto libre (notas): podrían contener datos personales y esta base es exportable.
CREATE TABLE stores (
    id   TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE CHECK (length(trim(name)) > 0)
);

CREATE TABLE products (
    id               TEXT PRIMARY KEY,
    sku              TEXT UNIQUE,
    name             TEXT NOT NULL CHECK (length(trim(name)) > 0),
    unit_price_cents INTEGER NOT NULL DEFAULT 0 CHECK (unit_price_cents >= 0),
    active           INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1))
);

CREATE TABLE orders (
    id                  TEXT PRIMARY KEY,
    store_id            TEXT NOT NULL REFERENCES stores(id),
    external_number     TEXT NOT NULL CHECK (length(trim(external_number)) > 0),
    status              TEXT NOT NULL CHECK (status IN ('pending','paid','preparing','shipped','delivered','cancelled')),
    order_date          TEXT NOT NULL,
    shipping_cost_cents INTEGER NOT NULL DEFAULT 0 CHECK (shipping_cost_cents >= 0),
    discount_cents      INTEGER NOT NULL DEFAULT 0 CHECK (discount_cents >= 0),
    clp_per_usd         REAL CHECK (clp_per_usd IS NULL OR clp_per_usd > 0),
    created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE (store_id, external_number)
);
CREATE INDEX idx_orders_status ON orders(status);
CREATE INDEX idx_orders_date   ON orders(order_date);

CREATE TABLE order_items (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    order_id         TEXT NOT NULL REFERENCES orders(id) ON DELETE CASCADE,
    product_id       TEXT REFERENCES products(id) ON DELETE SET NULL,
    name             TEXT NOT NULL CHECK (length(trim(name)) > 0),
    quantity         INTEGER NOT NULL CHECK (quantity > 0),
    unit_price_cents INTEGER NOT NULL CHECK (unit_price_cents >= 0)
);
CREATE INDEX idx_items_order ON order_items(order_id);

CREATE TABLE status_history (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    order_id    TEXT NOT NULL REFERENCES orders(id) ON DELETE CASCADE,
    from_status TEXT,
    to_status   TEXT NOT NULL,
    changed_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_history_order ON status_history(order_id);

CREATE TABLE shipments (
    order_id        TEXT PRIMARY KEY REFERENCES orders(id) ON DELETE CASCADE,
    carrier         TEXT,
    tracking_number TEXT,
    shipped_at      TEXT,
    delivered_at    TEXT
);

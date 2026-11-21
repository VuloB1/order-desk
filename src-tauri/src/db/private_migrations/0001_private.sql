-- Datos personales de cada pedido. Solo existen en private.db (SQLCipher).
-- order_id apunta a orders.id de business.db; al ser otra base no hay clave foranea:
-- las filas huerfanas se purgan al desbloquear.
CREATE TABLE order_private (
    order_id   TEXT PRIMARY KEY,
    name       TEXT,
    email      TEXT,
    phone      TEXT,
    address    TEXT,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- El tipo de cambio a CLP deja de usarse: los importes son solo USD.
ALTER TABLE orders DROP COLUMN clp_per_usd;

-- Código de descuento aplicado (copia del texto; el importe ya está en discount_cents).
ALTER TABLE orders ADD COLUMN discount_code TEXT;

-- Orden manual de los productos (botones rápidos). Se conserva el orden alfabético actual.
ALTER TABLE products ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;
UPDATE products SET sort_order = (
    SELECT COUNT(*) FROM products p2
    WHERE lower(p2.name) < lower(products.name)
       OR (lower(p2.name) = lower(products.name) AND p2.rowid < products.rowid)
);

-- percent: value = porcentaje entero (1–100). fixed: value = céntimos de USD.
CREATE TABLE discount_codes (
    id     TEXT PRIMARY KEY,
    code   TEXT NOT NULL UNIQUE COLLATE NOCASE CHECK (length(trim(code)) > 0),
    kind   TEXT NOT NULL CHECK (kind IN ('percent', 'fixed')),
    value  INTEGER NOT NULL CHECK (value > 0),
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    CHECK (kind <> 'percent' OR value <= 100)
);

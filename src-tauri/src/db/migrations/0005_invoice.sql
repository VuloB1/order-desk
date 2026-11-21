-- Factura del pedido (datos publicos: no identifican a la persona).
ALTER TABLE orders ADD COLUMN invoice_required INTEGER NOT NULL DEFAULT 0;
ALTER TABLE orders ADD COLUMN invoice_number TEXT;

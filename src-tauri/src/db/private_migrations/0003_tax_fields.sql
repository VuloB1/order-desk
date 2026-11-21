-- RUT y datos de empresa para factura (privados).
ALTER TABLE order_private ADD COLUMN rut TEXT;
ALTER TABLE order_private ADD COLUMN business_name TEXT;
ALTER TABLE order_private ADD COLUMN business_activity TEXT;

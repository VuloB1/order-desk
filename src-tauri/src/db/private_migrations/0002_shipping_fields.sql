-- Datos de envio y contacto del pedido (todo privado).
ALTER TABLE order_private ADD COLUMN country TEXT;
ALTER TABLE order_private ADD COLUMN region TEXT;
ALTER TABLE order_private ADD COLUMN city TEXT;
ALTER TABLE order_private ADD COLUMN postal_code TEXT;
ALTER TABLE order_private ADD COLUMN delivery_notes TEXT;

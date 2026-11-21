-- Destino del pedido: Chile o extranjero. Los pedidos existentes se consideran de Chile.
ALTER TABLE orders ADD COLUMN destination TEXT NOT NULL DEFAULT 'chile' CHECK (destination IN ('chile', 'foreign'));

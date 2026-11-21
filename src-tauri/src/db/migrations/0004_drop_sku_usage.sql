-- El SKU deja de usarse (la columna se conserva: SQLite no permite borrar columnas UNIQUE).
UPDATE products SET sku = NULL;

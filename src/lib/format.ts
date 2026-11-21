/** Solo para mostrar: antepone # sin duplicarlo. El número guardado no se toca. */
export const displayNumber = (n: string) => (n.startsWith("#") ? n : `#${n}`);

/**
 * Siguiente número de pedido: suma 1 al último grupo de dígitos y conserva el resto
 * (prefijo, sufijo y ceros a la izquierda). `null` si el número no tiene dígitos.
 */
export function nextOrderNumber(n: string): string | null {
  const m = /^(.*?)(\d+)(\D*)$/s.exec(n);
  if (!m) return null;
  const digits = m[2].split("");
  let i = digits.length - 1;
  while (i >= 0 && digits[i] === "9") digits[i--] = "0";
  if (i >= 0) digits[i] = String(Number(digits[i]) + 1);
  else digits.unshift("1");
  return m[1] + digits.join("") + m[3];
}

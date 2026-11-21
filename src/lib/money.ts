/** Dólares escritos por el usuario ("12", "12,5", "12.50") → céntimos; null si no es válido. */
export function parseUsd(s: string): number | null {
  const v = s.trim().replace(",", ".");
  if (!/^\d+(\.\d{1,2})?$/.test(v)) return null;
  const [int, dec = ""] = v.split(".");
  return Number(int) * 100 + Number(dec.padEnd(2, "0"));
}

export const centsToInput = (c: number) => (c / 100).toFixed(2);

export const formatUsd = (cents: number) =>
  new Intl.NumberFormat("en-US", { style: "currency", currency: "USD" }).format(cents / 100);

/** Fecha local de hoy como AAAA-MM-DD (toISOString daría la de UTC). */
export function todayIso(d = new Date()): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

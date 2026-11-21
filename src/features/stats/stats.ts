import type { Destination, OrderStatus, OrderSummary } from "@/lib/api";
import { DESTINATIONS, STATUSES } from "@/lib/api";

export interface MonthTotal {
  month: string; // AAAA-MM
  count: number;
  totalCents: number;
}

export interface Stats {
  orders: number;
  revenueCents: number;
  averageCents: number;
  invoicesPending: number;
  byStatus: Record<OrderStatus, number>;
  byDestination: Record<Destination, { count: number; totalCents: number }>;
  months: MonthTotal[];
}

/** Los `n` últimos meses hasta el de `today` (AAAA-MM-DD), el más reciente al final. */
export function lastMonths(today: string, n: number): string[] {
  const y = Number(today.slice(0, 4));
  const m = Number(today.slice(5, 7)) - 1;
  return Array.from({ length: n }, (_, i) => {
    const d = new Date(Date.UTC(y, m - (n - 1 - i), 1));
    return `${d.getUTCFullYear()}-${String(d.getUTCMonth() + 1).padStart(2, "0")}`;
  });
}

/** Cifras del negocio. Los pedidos cancelados cuentan en «por estado» pero no en ventas ni totales. */
export function summarize(orders: OrderSummary[], today: string, months = 6): Stats {
  const byStatus = Object.fromEntries(STATUSES.map((s) => [s, 0])) as Stats["byStatus"];
  const byDestination = Object.fromEntries(DESTINATIONS.map((d) => [d, { count: 0, totalCents: 0 }])) as Stats["byDestination"];
  const perMonth = new Map(lastMonths(today, months).map((m) => [m, { month: m, count: 0, totalCents: 0 }]));
  let count = 0, revenue = 0, pending = 0;
  for (const o of orders) {
    byStatus[o.status]++;
    if (o.status === "cancelled") continue;
    count++;
    revenue += o.totalCents;
    if (o.invoiceRequired && !o.invoiceNumber) pending++;
    byDestination[o.destination].count++;
    byDestination[o.destination].totalCents += o.totalCents;
    const m = perMonth.get(o.orderDate.slice(0, 7));
    if (m) { m.count++; m.totalCents += o.totalCents; }
  }
  return {
    orders: count,
    revenueCents: revenue,
    averageCents: count ? Math.round(revenue / count) : 0,
    invoicesPending: pending,
    byStatus,
    byDestination,
    months: [...perMonth.values()],
  };
}

import { useEffect, useMemo, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { api, DESTINATIONS, errorMessage, STATUSES, type OrderSummary, type ProductTotal } from "@/lib/api";
import { formatUsd, todayIso } from "@/lib/money";
import { destinationColor, destinationLabel, statusColor, statusLabel, t } from "@/i18n/es";
import { summarize } from "./stats";

const monthLabel = (m: string) =>
  new Date(Date.UTC(Number(m.slice(0, 4)), Number(m.slice(5, 7)) - 1, 1)).toLocaleDateString("es", { month: "short", year: "numeric", timeZone: "UTC" });

const Card = ({ label, value }: { label: string; value: string }) => (
  <div className="rounded-lg border p-3">
    <div className="text-xs text-muted-foreground">{label}</div>
    <div className="text-xl font-medium tabular-nums">{value}</div>
  </div>
);

/** Cifras del negocio. Solo usa datos públicos (business.db): no necesita la sección privada. */
export default function StatsDialog({ onClose }: { onClose: () => void }) {
  const [orders, setOrders] = useState<OrderSummary[] | null>(null);
  const [top, setTop] = useState<ProductTotal[]>([]);
  const [error, setError] = useState<string | null>(null);
  const s = useMemo(() => (orders ? summarize(orders, todayIso()) : null), [orders]);

  useEffect(() => {
    Promise.all([api.listOrders({}), api.topProducts(5)])
      .then(([o, p]) => { setOrders(o); setTop(p); })
      .catch((e) => setError(errorMessage(e)));
  }, []);

  const maxMonth = Math.max(1, ...(s?.months.map((m) => m.totalCents) ?? []));
  const x = t.stats;

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-2xl">
        <DialogHeader><DialogTitle>{x.title}</DialogTitle></DialogHeader>
        {error && <p role="alert" className="text-sm text-destructive">{error}</p>}
        {s && (
          <div className="space-y-5 text-sm">
            <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
              <Card label={x.orders} value={String(s.orders)} />
              <Card label={x.revenue} value={formatUsd(s.revenueCents)} />
              <Card label={x.average} value={formatUsd(s.averageCents)} />
              <Card label={x.invoicesPending} value={String(s.invoicesPending)} />
            </div>

            <section className="space-y-1.5">
              <h3 className="font-medium">{x.byMonth}</h3>
              {s.months.map((m) => (
                <div key={m.month} className="grid grid-cols-[5rem_1fr_9rem] items-center gap-2">
                  <span className="text-muted-foreground">{monthLabel(m.month)}</span>
                  <div className="h-2 rounded-full bg-muted">
                    <div className="h-2 rounded-full bg-primary" style={{ width: `${(m.totalCents / maxMonth) * 100}%` }} />
                  </div>
                  <span className="text-right tabular-nums">{formatUsd(m.totalCents)} · {m.count}</span>
                </div>
              ))}
            </section>

            <div className="grid gap-5 sm:grid-cols-2">
              <section className="space-y-1.5">
                <h3 className="font-medium">{x.byStatus}</h3>
                {STATUSES.map((st) => (
                  <div key={st} className="flex items-center justify-between">
                    <Badge className={statusColor[st]}>{statusLabel[st]}</Badge>
                    <span className="tabular-nums">{s.byStatus[st]}</span>
                  </div>
                ))}
              </section>
              <section className="space-y-1.5">
                <h3 className="font-medium">{x.byDestination}</h3>
                {DESTINATIONS.map((d) => (
                  <div key={d} className="flex items-center justify-between">
                    <Badge className={destinationColor[d]}>{destinationLabel[d]}</Badge>
                    <span className="tabular-nums">{s.byDestination[d].count} · {formatUsd(s.byDestination[d].totalCents)}</span>
                  </div>
                ))}
                <h3 className="pt-3 font-medium">{x.topProducts}</h3>
                {top.length === 0 && <p className="text-muted-foreground">{x.none}</p>}
                {top.map((p, i) => (
                  <div key={p.name} className="flex items-center justify-between gap-2">
                    <span className="truncate">{i + 1}. {p.name}</span>
                    <span className="shrink-0 tabular-nums text-muted-foreground">{p.quantity} · {formatUsd(p.totalCents)}</span>
                  </div>
                ))}
              </section>
            </div>
            <p className="text-xs text-muted-foreground">{x.note}</p>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}

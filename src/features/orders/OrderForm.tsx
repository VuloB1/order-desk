import { useEffect, useState } from "react";
import { LockOpen, Plus, Save, Trash2, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { DatePicker } from "@/components/ui/date-picker";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { api, errorMessage, type DiscountCode, type OrderDetail, type PrivateData, type Product } from "@/lib/api";
import { nextOrderNumber } from "@/lib/format";
import { centsToInput, formatUsd } from "@/lib/money";
import { destinationItems, statusItems, t } from "@/i18n/es";
import PrivateFields, { emptyPrivate, forDestination, hasPrivate } from "@/features/vault/PrivateFields";
import { useVault } from "@/features/vault/VaultContext";
import {
  addPreset, detailToDraft, draftToInput, draftTotals, emptyDraft, emptyLine, lineTotal, MAX_QUICK_QTY, type OrderDraft,
} from "./form";

interface Props {
  products: Product[];
  codes: DiscountCode[];
  editing?: OrderDetail;
  onClose: () => void;
  onSaved: (o: OrderDetail) => void;
}

// Se monta de nuevo cada vez que se abre, así el borrador siempre parte limpio.
export default function OrderForm({ products, codes, editing, onClose, onSaved }: Props) {
  const presets = products.filter((p) => p.active); // ya vienen en el orden guardado
  const [d, setD] = useState<OrderDraft>(() => (editing ? detailToDraft(editing, codes) : emptyDraft()));
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const { status: vault, openDialog } = useVault();
  const [priv, setPriv] = useState<PrivateData>(emptyPrivate); // datos privados del pedido nuevo
  const [created, setCreated] = useState<OrderDetail | null>(null); // pedido ya creado si falló el guardado privado
  const [privReady, setPrivReady] = useState(!editing); // al editar, hasta cargar lo guardado no se escribe nada
  const [qtyOpen, setQtyOpen] = useState<number | null>(null); // línea con la cuadrícula de cantidad abierta
  const totals = draftTotals(d);
  // Códigos activos, más el aplicado aunque ya esté desactivado (al editar un pedido antiguo).
  const chips = codes.filter((c) => c.active || c.id === d.code?.id);
  const set = (patch: Partial<OrderDraft>) => setD((p) => ({ ...p, ...patch }));
  const setLine = (i: number, patch: Partial<OrderDraft["items"][number]>) =>
    set({ items: d.items.map((l, j) => (j === i ? { ...l, ...patch } : l)) });

  // Pedido nuevo: propone el número siguiente al del último pedido añadido (si aún no se ha escrito nada).
  useEffect(() => {
    if (editing) return;
    api.lastOrderNumber().then((last) => {
      const next = last && nextOrderNumber(last);
      if (!next) return;
      setD((p) => (p.externalNumber ? p : { ...p, externalNumber: next }));
      // El campo ya tiene el foco: se selecciona el número propuesto para poder escribir encima.
      setTimeout(() => {
        const el = document.getElementById("num") as HTMLInputElement | null;
        if (el && document.activeElement === el && el.value === next) el.select();
      });
    }).catch(() => {}); // es solo una sugerencia
  }, [editing]);

  // Editar: con la sección desbloqueada se cargan los datos privados guardados para poder cambiarlos aquí.
  useEffect(() => {
    if (!editing || vault !== "unlocked") return;
    let live = true;
    api.getPrivateData(editing.id)
      .then((p) => { if (live) { setPriv(p ?? emptyPrivate); setPrivReady(true); } })
      .catch((e) => { if (live) setError(errorMessage(e)); });
    return () => { live = false; };
  }, [editing, vault]);

  async function save() {
    const r = draftToInput(d);
    if (!r.ok) return setError(r.error);
    setBusy(true);
    let orderSaved = false;
    try {
      const order = editing ? await api.updateOrder(editing.id, r.input) : (created ?? (await api.createOrder(r.input)));
      orderSaved = true;
      if (!editing) setCreated(order); // si el guardado privado falla, reintentar no duplica el pedido
      // Al editar se guarda siempre: dejar todos los campos vacíos borra los datos privados.
      if (vault === "unlocked" && privReady && (editing || hasPrivate(priv))) {
        await api.setPrivateData(order.id, forDestination(priv, d.destination));
      }
      onSaved(order);
    } catch (e) {
      setError(orderSaved ? `El pedido se guardó, pero los datos privados no: ${errorMessage(e)}` : errorMessage(e));
      setBusy(false);
    }
  }

  return (
    <Dialog
      open
      onOpenChange={(o, ev) => {
        if (o) return;
        // Un clic fuera por accidente no debe tirar el pedido a medio escribir.
        if (ev.reason === "outside-press") return ev.cancel();
        onClose();
      }}
    >
      <DialogContent className="flex max-h-[90vh] flex-col overflow-hidden sm:max-w-2xl">
        <DialogHeader className="flex-row items-center justify-between gap-3 pr-8">
          <DialogTitle>{editing ? t.editOrder : t.newOrder}</DialogTitle>
        </DialogHeader>

        {/* Solo esta zona se desplaza: cabecera, totales y botones quedan siempre a la vista. */}
        <div className="-mr-2 min-h-0 flex-1 space-y-4 overflow-y-auto pr-2">
        <div className="flex items-start gap-3">
          <div className="flex-1 space-y-1">
            <Label htmlFor="num">{t.number}</Label>
            <Input id="num" autoFocus onFocus={(e) => e.target.select()} placeholder="#1424" value={d.externalNumber} onChange={(e) => set({ externalNumber: e.target.value })} />
          </div>
          <div className="space-y-1">
            <Label htmlFor="date">{t.date}</Label>
            <DatePicker id="date" value={d.orderDate} onChange={(v) => set({ orderDate: v })} className="w-full" />
          </div>
          {!editing && (
            <div className="space-y-1">
              <Label>{t.status}</Label>
              <Select value={d.status} items={statusItems} onValueChange={(v) => set({ status: v as OrderDraft["status"] })}>
                <SelectTrigger className="w-36"><SelectValue /></SelectTrigger>
                <SelectContent>
                  {statusItems.map((s) => <SelectItem key={s.value} value={s.value}>{s.label}</SelectItem>)}
                </SelectContent>
              </Select>
            </div>
          )}
        </div>

        <div className="space-y-2">
          <Label>{t.items}</Label>
          <div className="sticky top-0 z-10 flex flex-wrap items-center gap-2 bg-popover py-1">
            <span className="text-xs text-muted-foreground">{t.quickAdd}:</span>
            {presets.map((p) => (
              <Button key={p.id} type="button" variant="secondary" size="sm" onClick={() => set({ items: addPreset(d.items, p) })}>
                <Plus data-icon="inline-start" />{p.name}
              </Button>
            ))}
            {presets.length === 0 && <span className="text-xs text-muted-foreground">{t.noPresets}</span>}
          </div>
          {d.items.map((l, i) => (
            <div key={i} className="grid grid-cols-[1fr_4rem_6.5rem_6rem_auto] items-center gap-2">
              <Input aria-label={t.product} placeholder={t.product} value={l.name} onChange={(e) => setLine(i, { name: e.target.value })} />
              <Popover open={qtyOpen === i} onOpenChange={(o) => setQtyOpen(o ? i : null)}>
                <PopoverTrigger render={<Button type="button" variant="outline" aria-label={`${t.quantity}: ${l.quantity}`} />}>
                  {l.quantity || "–"}
                </PopoverTrigger>
                <PopoverContent className="w-auto p-2" align="start">
                  <div className="grid grid-cols-5 gap-1">
                    {Array.from({ length: MAX_QUICK_QTY }, (_, n) => String(n + 1)).map((q) => (
                      <Button
                        key={q}
                        type="button"
                        size="sm"
                        variant={q === l.quantity ? "default" : "outline"}
                        className="w-9"
                        onClick={() => { setLine(i, { quantity: q }); setQtyOpen(null); }}
                      >
                        {q}
                      </Button>
                    ))}
                  </div>
                </PopoverContent>
              </Popover>
              <Input aria-label={t.unitPrice} placeholder="USD" inputMode="decimal" value={l.price} onChange={(e) => setLine(i, { price: e.target.value })} />
              <span aria-label={t.lineTotal} className="text-right text-sm tabular-nums">{formatUsd(lineTotal(l))}</span>
              <Button type="button" variant="ghost" size="sm" aria-label={t.removeLine} disabled={d.items.length === 1}
                onClick={() => set({ items: d.items.filter((_, j) => j !== i) })}><Trash2 /></Button>
            </div>
          ))}
          <Button type="button" variant="outline" size="sm" onClick={() => set({ items: [...d.items, emptyLine()] })}>
            <Plus data-icon="inline-start" />{t.customLine}
          </Button>
        </div>

        <div className="grid grid-cols-3 gap-3">
          <div className="space-y-1">
            <Label>{t.destination}</Label>
            <div className="flex gap-1" role="group" aria-label={t.destination}>
              {destinationItems.map((o) => (
                <Button
                  key={o.value}
                  type="button"
                  className="flex-1"
                  aria-pressed={d.destination === o.value}
                  variant={d.destination === o.value ? "default" : "outline"}
                  onClick={() => set({ destination: o.value })}
                >
                  {o.label}
                </Button>
              ))}
            </div>
          </div>
          <div className="space-y-1">
            <Label htmlFor="ship">{t.shipping}</Label>
            <Input id="ship" inputMode="decimal" value={d.shipping} onChange={(e) => set({ shipping: e.target.value })} />
          </div>
          <div className="space-y-1">
            <Label htmlFor="disc">{t.discount}</Label>
            <Input
              id="disc"
              inputMode="decimal"
              disabled={!!d.code}
              value={d.code ? centsToInput(totals.discount) : d.discount}
              onChange={(e) => set({ discount: e.target.value })}
            />
          </div>
        </div>
        {chips.length > 0 && (
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-xs text-muted-foreground">{t.discountCodes}:</span>
            {chips.map((c) => (
              <Button
                key={c.id}
                type="button"
                size="sm"
                aria-pressed={d.code?.id === c.id}
                variant={d.code?.id === c.id ? "default" : "secondary"}
                onClick={() => set({ code: d.code?.id === c.id ? null : c })}
              >
                {c.code} · {c.kind === "percent" ? `${c.value} %` : formatUsd(c.value)}
              </Button>
            ))}
          </div>
        )}
        <div className="grid grid-cols-3 gap-3">
          <div className="space-y-1">
            <Label>{t.invoice}</Label>
            <Button
              type="button"
              className="w-full"
              aria-pressed={d.invoiceRequired || !!d.invoiceNumber.trim()}
              variant={d.invoiceRequired || d.invoiceNumber.trim() ? "default" : "outline"}
              onClick={() => set({ invoiceRequired: !d.invoiceRequired, invoiceNumber: d.invoiceRequired ? "" : d.invoiceNumber })}
            >
              {t.invoiceRequired}
            </Button>
          </div>
          {(d.invoiceRequired || d.invoiceNumber.trim()) && (
            <div className="space-y-1">
              <Label htmlFor="invnum">{t.invoiceNumber}</Label>
              <Input id="invnum" autoComplete="off" value={d.invoiceNumber} onChange={(e) => set({ invoiceNumber: e.target.value })} />
            </div>
          )}
        </div>
        <div className="space-y-2 rounded-md border border-dashed border-amber-500/60 bg-amber-500/5 p-3">
            <div className="flex items-center gap-2">
              <Label>{t.vault.privateData}</Label>
              <span className="rounded-full bg-amber-500/20 px-2 py-0.5 text-xs text-amber-700 dark:text-amber-300">{t.vault.privateBadge}</span>
            </div>
            <p className="text-xs text-muted-foreground">{t.vault.privateFormHint}</p>
            {vault === "unlocked" && !privReady ? (
              <p className="text-sm text-muted-foreground">{t.vault.loading}</p>
            ) : vault === "unlocked" ? (
              <PrivateFields data={priv} onChange={setPriv} destination={d.destination} invoice={d.invoiceRequired || !!d.invoiceNumber.trim()} idPrefix="newpriv" />
            ) : (
              <div className="flex items-center gap-3">
                <p className="text-sm text-muted-foreground">{vault === "none" ? t.vault.noneHint : t.vault.lockedFormHint}</p>
                <Button type="button" variant="outline" size="sm" className="shrink-0" onClick={openDialog}><LockOpen data-icon="inline-start" />{vault === "none" ? t.vault.create : t.vault.unlock}</Button>
              </div>
            )}
          </div>
        </div>

        <div className="flex justify-end gap-6 text-sm">
          <span className="text-muted-foreground">{t.subtotal}: {formatUsd(totals.subtotal)}</span>
          {totals.discount > 0 && <span className="text-muted-foreground">{t.discountLabel}: −{formatUsd(totals.discount)}</span>}
          <span className="font-medium">{t.total}: {formatUsd(totals.total)}</span>
        </div>

        {error && <p role="alert" className="text-sm text-destructive">{error}</p>}
        <DialogFooter>
          <Button variant="outline" onClick={onClose}><X data-icon="inline-start" />{t.cancel}</Button>
          <Button onClick={save} disabled={busy}><Save data-icon="inline-start" />{t.save}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

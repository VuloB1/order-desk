import { useState } from "react";
import { FileUp } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { DatePicker } from "@/components/ui/date-picker";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { api, errorMessage, type ImportResult, type ImportRow, type OrderStatus } from "@/lib/api";
import { todayIso } from "@/lib/money";
import { destinationColor, destinationLabel, statusItems, t } from "@/i18n/es";

const SHOWN = 100;

/** Importa pedidos de un Excel: elegir fichero → resumen y valores por defecto → aplicar. */
export default function ImportDialog({ onClose, onChanged }: { onClose: () => void; onChanged: () => void }) {
  const [rows, setRows] = useState<ImportRow[] | null>(null);
  const [status, setStatus] = useState<OrderStatus>("shipped");
  const [date, setDate] = useState(todayIso());
  const [result, setResult] = useState<ImportResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const x = t.importer;

  const count = (a: ImportRow["action"]) => rows?.filter((r) => r.action === a).length ?? 0;
  const changes = rows?.filter((r) => r.action !== "same") ?? [];

  async function choose() {
    setError(null);
    setResult(null);
    try {
      const r = await api.importPreview();
      if (r) setRows(r);
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  async function apply() {
    if (!rows) return;
    setBusy(true);
    setError(null);
    try {
      setResult(await api.importApply(rows, status, date));
      setRows(null);
      onChanged();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-2xl">
        <DialogHeader><DialogTitle>{x.title}</DialogTitle></DialogHeader>
        <div className="space-y-4 text-sm">
          {!rows && <p className="text-muted-foreground">{x.intro}</p>}
          {error && <p role="alert" className="text-destructive">{error}</p>}
          {result && (
            <div className="space-y-1">
              <p role="status">{x.done(result.created, result.updated, result.skipped)}</p>
              {result.errors.map((e) => <p key={e} className="text-destructive">{e}</p>)}
            </div>
          )}

          {rows && (
            <>
              <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
                {(["new", "update", "same", "skip"] as const).map((a) => (
                  <div key={a} className="rounded-lg border p-3">
                    <div className="text-xs text-muted-foreground">{x[a]}</div>
                    <div className="text-xl font-medium tabular-nums">{count(a)}</div>
                  </div>
                ))}
              </div>

              {count("new") > 0 && (
                <div className="space-y-2">
                  <p>{x.newDefaults}</p>
                  <div className="flex flex-wrap items-end gap-4">
                    <div className="space-y-1.5">
                      <Label>{x.status}</Label>
                      <Select value={status} items={statusItems} onValueChange={(v) => setStatus(v as OrderStatus)}>
                        <SelectTrigger className="w-36"><SelectValue /></SelectTrigger>
                        <SelectContent>
                          {statusItems.map((s) => <SelectItem key={s.value} value={s.value}>{s.label}</SelectItem>)}
                        </SelectContent>
                      </Select>
                    </div>
                    <div className="space-y-1.5">
                      <Label>{x.date}</Label>
                      <DatePicker aria-label={x.date} value={date} onChange={(v) => v && setDate(v)} />
                    </div>
                  </div>
                </div>
              )}

              {changes.length > 0 && (
                <section className="space-y-1.5">
                  <h3 className="font-medium">{x.changes}</h3>
                  <div className="max-h-64 space-y-1 overflow-y-auto rounded-md border p-2">
                    {changes.slice(0, SHOWN).map((r) => (
                      <div key={r.row} className="flex flex-wrap items-center gap-2">
                        <span className="w-20 shrink-0 tabular-nums text-muted-foreground">{x.row} {r.row}</span>
                        <span className="font-medium">{r.number}</span>
                        <span className="text-muted-foreground">{r.kit}</span>
                        <Badge className={destinationColor[r.destination]}>{destinationLabel[r.destination]}</Badge>
                        <span className="text-xs">{x[r.action]}{r.note ? ` · ${r.note}` : ""}</span>
                      </div>
                    ))}
                    {changes.length > SHOWN && <p className="text-xs text-muted-foreground">{x.moreRows(changes.length - SHOWN)}</p>}
                  </div>
                </section>
              )}
              {count("new") + count("update") === 0 && <p className="text-muted-foreground">{x.nothing}</p>}
            </>
          )}

          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={() => void choose()} disabled={busy}><FileUp data-icon="inline-start" />{rows ? x.another : x.choose}</Button>
            {rows && <Button onClick={() => void apply()} disabled={busy || count("new") + count("update") === 0}>{x.apply}</Button>}
            <Button variant="ghost" onClick={onClose}>{t.close}</Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

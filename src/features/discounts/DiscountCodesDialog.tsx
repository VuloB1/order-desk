import { useState } from "react";
import { Pencil, Plus, Save, Trash2, X } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import ConfirmDialog from "@/components/ConfirmDialog";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { api, errorMessage, type DiscountCode } from "@/lib/api";
import { centsToInput, formatUsd, parseUsd } from "@/lib/money";
import { t } from "@/i18n/es";

interface Props {
  codes: DiscountCode[];
  onClose: () => void;
  onChanged: () => void;
}

interface Draft {
  id: string | null; // null = código nuevo
  code: string;
  kind: DiscountCode["kind"];
  value: string; // % entero, o USD si es fijo
  active: boolean;
}

const kindItems = [
  { value: "percent", label: t.kindPercent },
  { value: "fixed", label: t.kindFixed },
];

const describe = (c: DiscountCode) => (c.kind === "percent" ? `${c.value} %` : formatUsd(c.value));

export default function DiscountCodesDialog({ codes, onClose, onChanged }: Props) {
  const [d, setD] = useState<Draft | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function remove() {
    if (!d?.id) return;
    setConfirmDelete(false);
    try {
      await api.deleteDiscountCode(d.id);
      setD(null);
      setError(null);
      onChanged();
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  async function save() {
    if (!d) return;
    if (!d.code.trim()) return setError(t.codeRequired);
    const value = d.kind === "percent" ? (/^\d+$/.test(d.value.trim()) ? Number(d.value) : null) : parseUsd(d.value);
    if (value == null || value < 1 || (d.kind === "percent" && value > 100)) return setError(t.codeValueInvalid);
    try {
      if (d.id) await api.updateDiscountCode({ id: d.id, code: d.code, kind: d.kind, value, active: d.active });
      else await api.createDiscountCode(d.code, d.kind, value);
      setD(null);
      setError(null);
      onChanged();
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  const edit = (c: DiscountCode) =>
    setD({
      id: c.id,
      code: c.code,
      kind: c.kind,
      value: c.kind === "percent" ? String(c.value) : centsToInput(c.value),
      active: c.active,
    });

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>{t.codesTitle}</DialogTitle>
        </DialogHeader>
        <p className="text-sm text-muted-foreground">{t.codesHint}</p>

        <ul className="divide-y rounded-md border">
          {codes.map((c) => (
            <li key={c.id} className="row-hover flex items-center gap-3 px-3 py-2 text-sm">
              <span className={c.active ? "font-medium" : "text-muted-foreground line-through"}>{c.code}</span>
              {!c.active && <Badge variant="secondary">{t.inactive}</Badge>}
              <span className="ml-auto">{describe(c)}</span>
              <Button variant="ghost" size="sm" onClick={() => edit(c)}><Pencil data-icon="inline-start" />{t.edit}</Button>
            </li>
          ))}
          {codes.length === 0 && <li className="px-3 py-4 text-center text-sm text-muted-foreground">{t.noCodes}</li>}
        </ul>

        <Button className="self-start" onClick={() => setD({ id: null, code: "", kind: "percent", value: "", active: true })}>
          <Plus data-icon="inline-start" />{t.newCode}
        </Button>

        {error && !d && <p role="alert" className="text-sm text-destructive">{error}</p>}

        <Dialog open={d !== null} onOpenChange={(o) => { if (!o) { setD(null); setError(null); } }}>
          <DialogContent className="sm:max-w-md">
            <DialogHeader>
              <DialogTitle>{d?.id ? t.edit : t.newCode}</DialogTitle>
            </DialogHeader>
            {d && (
              <div className="space-y-3">
                <div className="space-y-1">
                  <Label htmlFor="ccode">{t.code}</Label>
                  <Input id="ccode" autoFocus value={d.code} onChange={(e) => setD({ ...d, code: e.target.value })} />
                </div>
                <div className="grid grid-cols-[1fr_7rem] gap-2">
                  <div className="space-y-1">
                    <Label>{t.codeKind}</Label>
                    <Select value={d.kind} items={kindItems} onValueChange={(v) => setD({ ...d, kind: (v ?? "percent") as Draft["kind"], value: "" })}>
                      <SelectTrigger><SelectValue /></SelectTrigger>
                      <SelectContent>
                        {kindItems.map((k) => <SelectItem key={k.value} value={k.value}>{k.label}</SelectItem>)}
                      </SelectContent>
                    </Select>
                  </div>
                  <div className="space-y-1">
                    <Label htmlFor="cvalue">{t.codeValue}</Label>
                    <Input id="cvalue" inputMode="decimal" value={d.value} onChange={(e) => setD({ ...d, value: e.target.value })} />
                  </div>
                </div>
                {d.id && (
                  <label className="flex items-center gap-2 text-sm">
                    <input type="checkbox" checked={d.active} onChange={(e) => setD({ ...d, active: e.target.checked })} />
                    {t.activeCodeHint}
                  </label>
                )}
                {error && <p role="alert" className="text-sm text-destructive">{error}</p>}
                <div className="flex items-center gap-2">
                  {d.id && <Button variant="destructive" onClick={() => setConfirmDelete(true)}><Trash2 data-icon="inline-start" />{t.deleteCode}</Button>}
                  <div className="ml-auto flex gap-2">
                    <Button variant="outline" onClick={() => { setD(null); setError(null); }}><X data-icon="inline-start" />{t.cancel}</Button>
                    <Button onClick={save}><Save data-icon="inline-start" />{t.save}</Button>
                  </div>
                </div>
              </div>
            )}
            <ConfirmDialog
              open={confirmDelete}
              onOpenChange={setConfirmDelete}
              title={t.deleteCodeTitle}
              body={t.deleteCodeBody}
              confirmLabel={t.deleteCode}
              onConfirm={() => void remove()}
            />
          </DialogContent>
        </Dialog>
      </DialogContent>
    </Dialog>
  );
}

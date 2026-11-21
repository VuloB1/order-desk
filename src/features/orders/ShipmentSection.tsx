import { useEffect, useState } from "react";
import { Pencil, Save, X } from "lucide-react";
import InfoRow from "@/components/InfoRow";
import { Button } from "@/components/ui/button";
import { DatePicker, formatDate } from "@/components/ui/date-picker";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { Shipment } from "@/lib/api";
import { t } from "@/i18n/es";

const empty: Shipment = { carrier: null, trackingNumber: null, shippedAt: null, deliveredAt: null };
const hasData = (s: Shipment | null) => !!s && Object.values(s).some(Boolean);

interface Props {
  shipment: Shipment | null;
  /** Devuelve true si se guardó. */
  onSave: (s: Shipment) => Promise<boolean>;
}

/** Datos de envío: se muestran como ficha y se editan solo al pulsar "Editar". */
export default function ShipmentSection({ shipment, onSave }: Props) {
  const [draft, setDraft] = useState<Shipment>(shipment ?? empty);
  const [editing, setEditing] = useState(!hasData(shipment));

  useEffect(() => {
    setDraft(shipment ?? empty);
    setEditing(!hasData(shipment));
  }, [shipment]);

  const text = (k: "carrier" | "trackingNumber") => ({
    value: draft[k] ?? "",
    onChange: (e: React.ChangeEvent<HTMLInputElement>) => setDraft({ ...draft, [k]: e.target.value || null }),
  });

  return (
    <div className="space-y-2 rounded-md border p-3">
      <div className="flex items-center justify-between">
        <Label>{t.shipment}</Label>
        {!editing && (
          <Button variant="ghost" size="sm" onClick={() => setEditing(true)}><Pencil data-icon="inline-start" />{t.edit}</Button>
        )}
      </div>

      {editing ? (
        <>
          <div className="grid grid-cols-2 gap-2">
            <div className="space-y-1"><Label className="text-xs">{t.carrier}</Label><Input aria-label={t.carrier} {...text("carrier")} /></div>
            <div className="space-y-1"><Label className="text-xs">{t.tracking}</Label><Input aria-label={t.tracking} {...text("trackingNumber")} /></div>
            <div className="space-y-1">
              <Label className="text-xs">{t.shippedAt}</Label>
              <DatePicker clearable className="w-full" aria-label={t.shippedAt} value={draft.shippedAt ?? ""} onChange={(v) => setDraft({ ...draft, shippedAt: v || null })} />
            </div>
            <div className="space-y-1">
              <Label className="text-xs">{t.deliveredAt}</Label>
              <DatePicker clearable className="w-full" aria-label={t.deliveredAt} value={draft.deliveredAt ?? ""} onChange={(v) => setDraft({ ...draft, deliveredAt: v || null })} />
            </div>
          </div>
          <div className="flex justify-end gap-2">
            {hasData(shipment) && (
              <Button variant="outline" size="sm" onClick={() => { setDraft(shipment ?? empty); setEditing(false); }}><X data-icon="inline-start" />{t.cancel}</Button>
            )}
            <Button size="sm" onClick={async () => { if (await onSave(draft)) setEditing(!hasData(draft)); }}><Save data-icon="inline-start" />{t.save}</Button>
          </div>
        </>
      ) : (
        <div className="space-y-1">
          {shipment?.carrier && <InfoRow label={t.carrier}>{shipment.carrier}</InfoRow>}
          {shipment?.trackingNumber && <InfoRow label={t.tracking}>{shipment.trackingNumber}</InfoRow>}
          {shipment?.shippedAt && <InfoRow label={t.shippedAt}>{formatDate(shipment.shippedAt)}</InfoRow>}
          {shipment?.deliveredAt && <InfoRow label={t.deliveredAt}>{formatDate(shipment.deliveredAt)}</InfoRow>}
        </div>
      )}
    </div>
  );
}

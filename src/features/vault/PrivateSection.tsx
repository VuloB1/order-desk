import { useEffect, useState } from "react";
import { LockOpen, Pencil, Save, X } from "lucide-react";
import InfoRow from "@/components/InfoRow";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { api, errorMessage, type Destination, type PrivateData } from "@/lib/api";
import { destinationLabel, t } from "@/i18n/es";
import PrivateFields, { emptyPrivate, forDestination } from "./PrivateFields";
import { useVault } from "./VaultContext";

const label = t.vault;

/** País tal como se muestra: avión + país en el extranjero; camión + Chile en Chile. */
const placeLine = (d: PrivateData, destination: Destination) =>
  destination === "chile" ? destinationLabel.chile : `✈️ ${d.country ?? ""}`.trim();

/** Datos personales del pedido. Solo se cargan con la sección desbloqueada y se vacían al bloquear. */
export default function PrivateSection({ orderId, destination, invoice }: { orderId: string; destination: Destination; invoice: boolean }) {
  const { status, openDialog } = useVault();
  const [saved, setSaved] = useState<PrivateData | null>(null); // null = aún sin datos
  const [data, setData] = useState<PrivateData>(emptyPrivate); // borrador del formulario
  const [editing, setEditing] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    setMsg(null);
    setSaved(null);
    setData(emptyPrivate); // al bloquear no queda nada en la memoria de la interfaz
    setEditing(false);
    if (status !== "unlocked") return;
    let live = true;
    api
      .getPrivateData(orderId)
      .then((d) => {
        if (!live) return;
        setSaved(d);
        setData(d ?? emptyPrivate);
        setEditing(d === null);
      })
      .catch((e) => { if (live) { setFailed(true); setMsg(errorMessage(e)); } });
    return () => { live = false; };
  }, [orderId, status]);

  async function save() {
    setFailed(false);
    try {
      await api.setPrivateData(orderId, forDestination(data, destination));
      const d = await api.getPrivateData(orderId); // lo guardado, ya normalizado por el backend
      setSaved(d);
      setData(d ?? emptyPrivate);
      setEditing(d === null);
      setMsg(d ? t.vault.savedData : null);
    } catch (e) {
      setFailed(true);
      setMsg(errorMessage(e));
    }
  }

  const header = (
    <div className="flex items-center justify-between">
      <Label>{label.privateData}</Label>
      {status === "unlocked" && !editing && saved && (
        <Button variant="ghost" size="sm" onClick={() => { setMsg(null); setEditing(true); }}><Pencil data-icon="inline-start" />{t.edit}</Button>
      )}
    </div>
  );

  if (status !== "unlocked") {
    return (
      <div className="space-y-2 rounded-md border p-3">
        {header}
        <div className="flex items-center gap-3">
          <p className="text-sm text-muted-foreground">{status === "none" ? label.noneHint : label.lockedHint}</p>
          <Button variant="outline" size="sm" className="shrink-0" onClick={openDialog}><LockOpen data-icon="inline-start" />{status === "none" ? label.create : label.unlock}</Button>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-2 rounded-md border p-3">
      {header}
      {editing ? (
        <>
          <div className="mb-1 text-sm font-medium">{placeLine(data, destination).replace(/ $/, "")}</div>
          <PrivateFields data={data} onChange={setData} destination={destination} invoice={invoice} />
          <div className="flex justify-end gap-2">
            {saved && (
              <Button variant="outline" size="sm" onClick={() => { setData(saved); setEditing(false); setMsg(null); }}><X data-icon="inline-start" />{t.cancel}</Button>
            )}
            <Button size="sm" onClick={() => void save()}><Save data-icon="inline-start" />{t.save}</Button>
          </div>
        </>
      ) : (
        saved && (
          <div className="space-y-1">
            <div className="text-sm font-medium">{placeLine(saved, destination)}</div>
            {saved.name && <InfoRow label={label.name}>{saved.name}</InfoRow>}
            {saved.rut && <InfoRow label={label.rut}>{saved.rut}</InfoRow>}
            {saved.businessName && <InfoRow label={label.businessName}>{saved.businessName}</InfoRow>}
            {saved.businessActivity && <InfoRow label={label.businessActivity}>{saved.businessActivity}</InfoRow>}
            {saved.email && <InfoRow label={label.email}>{saved.email}</InfoRow>}
            {saved.phone && <InfoRow label={label.phone}>{saved.phone}</InfoRow>}
            {saved.region && <InfoRow label={label.region}>{saved.region}</InfoRow>}
            {saved.city && <InfoRow label={label.city}>{saved.city}</InfoRow>}
            {saved.address && <InfoRow label={label.address}>{saved.address}</InfoRow>}
            {saved.postalCode && <InfoRow label={label.postalCode}>{saved.postalCode}</InfoRow>}
            {saved.deliveryNotes && <InfoRow label={label.deliveryNotes}>{saved.deliveryNotes}</InfoRow>}
          </div>
        )
      )}
      {msg && <p role={failed ? "alert" : "status"} className={`text-sm ${failed ? "text-destructive" : ""}`}>{msg}</p>}
    </div>
  );
}

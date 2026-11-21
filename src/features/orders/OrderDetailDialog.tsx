import { useEffect, useState } from "react";
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription,
  AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Pencil, Save, Trash2, X } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { api, errorMessage, STATUSES, type OrderDetail, type OrderStatus } from "@/lib/api";
import { displayNumber } from "@/lib/format";
import { formatUsd } from "@/lib/money";
import PrivateSection from "@/features/vault/PrivateSection";
import ShipmentSection from "./ShipmentSection";
import { destinationColor, destinationLabel, invoiceColor, statusColor, statusItems, statusLabel, t } from "@/i18n/es";

interface Props {
  id: string;
  onClose: () => void;
  onChanged: () => void;
  onEdit: (o: OrderDetail) => void;
}

export default function OrderDetailDialog({ id, onClose, onChanged, onEdit }: Props) {
  const [o, setO] = useState<OrderDetail | null>(null);
  const [next, setNext] = useState<OrderStatus | "">("");
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const apply = (d: OrderDetail) => {
    setO(d);
    setNext("");
    setError(null);
  };
  const run = async (p: Promise<OrderDetail>): Promise<boolean> => {
    try {
      apply(await p);
      onChanged();
      return true;
    } catch (e) {
      setError(errorMessage(e));
      return false;
    }
  };

  useEffect(() => {
    api.getOrder(id).then(apply).catch((e) => setError(errorMessage(e)));
  }, [id]);

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>{o ? displayNumber(o.externalNumber) : "…"}</DialogTitle>
        </DialogHeader>
        {error && <p role="alert" className="text-sm text-destructive">{error}</p>}
        {o && (
          <div className="space-y-5 text-sm">
            <div className="flex flex-wrap items-center gap-2 text-muted-foreground">
              <span>📅 {o.orderDate}</span>
              <Badge className={statusColor[o.status]}>{statusLabel[o.status]}</Badge>
              <Badge className={destinationColor[o.destination]}>{destinationLabel[o.destination]}</Badge>
              {o.invoiceRequired && (
                <Badge className={o.invoiceNumber ? invoiceColor.issued : invoiceColor.pending}>
                  {o.invoiceNumber ? t.invoiceIssued(o.invoiceNumber) : t.invoicePending}
                </Badge>
              )}
            </div>

            <table className="w-full">
              <tbody>
                {o.items.map((i, n) => (
                  <tr key={n} className="border-b">
                    <td className="py-1">{i.name}</td>
                    <td className="text-right">{i.quantity} × {formatUsd(i.unitPriceCents)}</td>
                    <td className="text-right">{formatUsd(i.quantity * i.unitPriceCents)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            <div className="space-y-0.5 text-right">
              <div className="text-muted-foreground">{t.shippingLabel}: {formatUsd(o.shippingCostCents)}</div>
              {o.discountCents > 0 && (
                <div className="text-muted-foreground">
                  {t.discountLabel}{o.discountCode ? ` (${o.discountCode})` : ""}: −{formatUsd(o.discountCents)}
                </div>
              )}
              <div className="font-medium">{t.total}: {formatUsd(o.totalCents)}</div>
            </div>

            <div className="space-y-1">
              <Label>{t.changeStatus}</Label>
              <div className="flex items-center gap-2">
                <Select value={next || null} items={statusItems} onValueChange={(v) => setNext((v ?? "") as OrderStatus | "")}>
                  <SelectTrigger className="w-48"><SelectValue placeholder={statusLabel[o.status]} /></SelectTrigger>
                  <SelectContent>
                    {STATUSES.filter((s) => s !== o.status).map((s) => <SelectItem key={s} value={s}>{statusLabel[s]}</SelectItem>)}
                  </SelectContent>
                </Select>
                <Button disabled={!next} onClick={() => next && run(api.changeStatus(o.id, next))}><Save data-icon="inline-start" />{t.save}</Button>
              </div>
            </div>

            <ShipmentSection shipment={o.shipment} onSave={(sh) => run(api.setShipment(o.id, sh))} />

            <PrivateSection orderId={o.id} destination={o.destination} invoice={o.invoiceRequired} />

            <div>
              <Label>{t.history}</Label>
              <ul className="mt-1 space-y-0.5 text-muted-foreground">
                {o.history.map((h, n) => (
                  <li key={n}>
                    {h.changedAt.slice(0, 16).replace("T", " ")} — {h.fromStatus ? `${statusLabel[h.fromStatus]} → ` : ""}{statusLabel[h.toStatus]}
                  </li>
                ))}
              </ul>
            </div>

            <div className="flex justify-between">
              <Button variant="destructive" onClick={() => setConfirmDelete(true)}><Trash2 data-icon="inline-start" />{t.delete}</Button>
              <div className="flex gap-2">
                <Button variant="outline" onClick={() => onEdit(o)}><Pencil data-icon="inline-start" />{t.edit}</Button>
                <Button variant="outline" onClick={onClose}><X data-icon="inline-start" />{t.close}</Button>
              </div>
            </div>
          </div>
        )}

        <AlertDialog open={confirmDelete} onOpenChange={setConfirmDelete}>
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>{t.deleteTitle}</AlertDialogTitle>
              <AlertDialogDescription>{t.deleteBody}</AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel>{t.cancel}</AlertDialogCancel>
              <AlertDialogAction
                onClick={async () => {
                  try {
                    await api.deleteOrder(id);
                    onChanged();
                    onClose();
                  } catch (e) {
                    setError(errorMessage(e));
                  }
                }}
              >
                {t.delete}
              </AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      </DialogContent>
    </Dialog>
  );
}

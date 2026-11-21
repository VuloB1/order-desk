import { memo } from "react";
import { Badge } from "@/components/ui/badge";
import { TableCell, TableRow } from "@/components/ui/table";
import type { OrderSummary } from "@/lib/api";
import { displayNumber } from "@/lib/format";
import { formatUsd } from "@/lib/money";
import { destinationColor, destinationLabel, invoiceColor, statusColor, statusLabel, t } from "@/i18n/es";

/** Fila de la lista. Con `memo`, abrir una ventana o escribir en un filtro no repinta las 1.000 filas. */
const OrderRow = memo(function OrderRow({ order: o, onOpen }: { order: OrderSummary; onOpen: (id: string) => void }) {
  return (
    <TableRow className="cursor-pointer" onClick={() => onOpen(o.id)}>
      <TableCell className="font-medium">{displayNumber(o.externalNumber)}</TableCell>
      <TableCell>{o.orderDate}</TableCell>
      <TableCell><Badge className={destinationColor[o.destination]}>{destinationLabel[o.destination]}</Badge></TableCell>
      <TableCell><Badge className={statusColor[o.status]}>{statusLabel[o.status]}</Badge></TableCell>
      <TableCell>
        {o.invoiceRequired && (
          <Badge className={o.invoiceNumber ? invoiceColor.issued : invoiceColor.pending}>
            {o.invoiceNumber ? t.invoiceIssued(o.invoiceNumber) : t.invoicePending}
          </Badge>
        )}
      </TableCell>
      <TableCell className="text-right">{o.itemCount}</TableCell>
      <TableCell className="text-right">{formatUsd(o.totalCents)}</TableCell>
    </TableRow>
  );
});

export default OrderRow;

import { useEffect, useState } from "react";
import { DatabaseBackup } from "lucide-react";
import { Button } from "@/components/ui/button";
import { api } from "@/lib/api";
import { t } from "@/i18n/es";
import BackupDialog from "./BackupDialog";
import { backupIsStale, lastBackup } from "./reminder";

/** Aviso suave bajo la cabecera: hay pedidos y la última copia es antigua (o no hay ninguna). */
export default function BackupReminder() {
  const [show, setShow] = useState(false);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    if (!backupIsStale(lastBackup())) return;
    api.listOrders({}).then((o) => setShow(o.length > 0)).catch(() => {});
    const hide = () => setShow(false);
    window.addEventListener("backup-done", hide);
    return () => window.removeEventListener("backup-done", hide);
  }, []);

  if (!show) return null;
  return (
    <div className="mx-auto max-w-6xl px-6 pt-4">
      <div className="flex flex-wrap items-center gap-3 rounded-md border border-amber-500/60 bg-amber-500/10 p-3 text-sm">
        <span className="flex-1">{t.backup.reminder}</span>
        <Button size="sm" onClick={() => setOpen(true)}><DatabaseBackup data-icon="inline-start" />{t.backup.reminderAction}</Button>
        <Button size="sm" variant="outline" onClick={() => setShow(false)}>{t.backup.reminderLater}</Button>
      </div>
      {open && <BackupDialog onClose={() => setOpen(false)} />}
    </div>
  );
}

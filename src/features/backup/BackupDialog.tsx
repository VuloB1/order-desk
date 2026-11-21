import { useState } from "react";
import { DatabaseBackup, HardDriveDownload, LockOpen, RotateCcw } from "lucide-react";
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription,
  AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { api, errorMessage } from "@/lib/api";
import { todayIso } from "@/lib/money";
import { t } from "@/i18n/es";
import { useVault } from "@/features/vault/VaultContext";
import { markBackup } from "./reminder";

type Kind = "password" | "recovery";

export default function BackupDialog({ onClose }: { onClose: () => void }) {
  const { status, openDialog } = useVault();
  const [msg, setMsg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [kind, setKind] = useState<Kind>("password");
  const [secret, setSecret] = useState("");
  const [confirm, setConfirm] = useState(false);
  const [done, setDone] = useState(false); // restauración terminada: hay que recargar la interfaz
  const [busy, setBusy] = useState(false);

  async function create() {
    setBusy(true);
    setError(null);
    setMsg(null);
    try {
      const r = await api.backupCreate(`order-desk-${todayIso()}.odbak`);
      if (r) markBackup();
      if (r) setMsg(t.backup.created(r.path, Math.max(1, Math.round(r.bytes / 1024))));
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function restore() {
    setConfirm(false);
    setBusy(true);
    setError(null);
    setMsg(null);
    try {
      const r = await api.backupRestore(kind, secret);
      if (r) {
        setMsg(t.backup.restored(r.orders));
        setDone(true);
      }
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setSecret(""); // la credencial no se queda en memoria de la interfaz
      setBusy(false);
    }
  }

  return (
    <Dialog open onOpenChange={(open, ev) => {
      if (open) return;
      if (done || ev?.reason === "outside-press") return ev?.cancel();
      onClose();
    }}>
      <DialogContent className="sm:max-w-lg" showCloseButton={!done}>
        <DialogHeader><DialogTitle>{t.backup.title}</DialogTitle></DialogHeader>

        <section className="space-y-2 rounded-md border p-3">
          <h3 className="text-sm font-medium">{t.backup.createTitle}</h3>
          <p className="text-sm text-muted-foreground">{t.backup.createHint}</p>
          {status === "unlocked" ? (
            <Button disabled={busy || done} onClick={() => void create()}><HardDriveDownload data-icon="inline-start" />{t.backup.create}</Button>
          ) : (
            <div className="flex items-center gap-3">
              <p className="text-sm text-muted-foreground">{t.backup.needUnlock}</p>
              <Button variant="outline" size="sm" className="shrink-0" onClick={openDialog}><LockOpen data-icon="inline-start" />{status === "none" ? t.vault.create : t.backup.unlock}</Button>
            </div>
          )}
        </section>

        <section className="space-y-2 rounded-md border p-3">
          <h3 className="text-sm font-medium">{t.backup.restoreTitle}</h3>
          <p className="text-sm text-muted-foreground">{t.backup.restoreHint}</p>
          <div className="flex gap-2">
            <Button variant={kind === "password" ? "default" : "outline"} size="sm" onClick={() => { setKind("password"); setSecret(""); }}>{t.backup.withPassword}</Button>
            <Button variant={kind === "recovery" ? "default" : "outline"} size="sm" onClick={() => { setKind("recovery"); setSecret(""); }}>{t.backup.withRecovery}</Button>
          </div>
          <Input
            aria-label={t.backup.secret}
            placeholder={t.backup.secret}
            type={kind === "password" ? "password" : "text"}
            autoComplete="off"
            className={kind === "recovery" ? "font-mono" : ""}
            value={secret}
            onChange={(e) => setSecret(e.target.value)}
          />
          <Button variant="outline" disabled={busy || done || !secret} onClick={() => setConfirm(true)}><DatabaseBackup data-icon="inline-start" />{t.backup.restore}</Button>
        </section>

        {msg && <p role="status" className="text-sm break-all">{msg}</p>}
        {error && <p role="alert" className="text-sm text-destructive">{error}</p>}
        {done && (
          <div className="flex justify-end">
            <Button onClick={() => window.location.reload()}><RotateCcw data-icon="inline-start" />{t.backup.continue}</Button>
          </div>
        )}

        <AlertDialog open={confirm} onOpenChange={setConfirm}>
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>{t.backup.confirmTitle}</AlertDialogTitle>
              <AlertDialogDescription>{t.backup.confirmBody}</AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel>{t.cancel}</AlertDialogCancel>
              <AlertDialogAction onClick={() => void restore()}>{t.backup.confirm}</AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      </DialogContent>
    </Dialog>
  );
}

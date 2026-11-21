import { useRef, useState, type FormEvent } from "react";
import { Check, Copy, KeyRound, Lock, LockOpen, Save, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { DepthContext, Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { api, errorMessage, type VaultStatus } from "@/lib/api";
import { t } from "@/i18n/es";

interface Props {
  status: VaultStatus;
  setStatus: (s: VaultStatus) => void;
  onClose: () => void;
}

type View = "main" | "recover" | "change";
const MIN = 10;

function PasswordField({ id, label, value, onChange, autoFocus }: {
  id: string; label: string; value: string; onChange: (v: string) => void; autoFocus?: boolean;
}) {
  return (
    <div className="space-y-1">
      <Label htmlFor={id}>{label}</Label>
      <Input id={id} type="password" autoComplete="off" autoFocus={autoFocus} value={value} onChange={(e) => onChange(e.target.value)} />
    </div>
  );
}

export default function VaultDialog({ status, setStatus, onClose }: Props) {
  const [view, setView] = useState<View>("main");
  const [pw, setPw] = useState("");
  const [pw2, setPw2] = useState("");
  const [oldPw, setOldPw] = useState("");
  const [recovery, setRecovery] = useState("");
  const [shownKey, setShownKey] = useState<string | null>(null); // clave de recuperación recién creada
  const [copied, setCopied] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const clear = () => { setPw(""); setPw2(""); setOldPw(""); setRecovery(""); };

  /** Ejecuta una operación; las contraseñas se borran del estado al terminar, bien o mal. */
  async function run(fn: () => Promise<void>) {
    setBusy(true);
    setError(null);
    setInfo(null);
    try {
      await fn();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      clear();
      setBusy(false);
    }
  }

  function checkNew(a: string, b: string): string | null {
    if (a.length < MIN) return t.vault.tooShort;
    return a === b ? null : t.vault.mismatch;
  }

  const submit = (fn: () => Promise<void>) => (e: FormEvent) => {
    e.preventDefault();
    void run(fn);
  };

  const create = submit(async () => {
    const bad = checkNew(pw, pw2);
    if (bad) throw bad;
    setShownKey(await api.vaultCreate(pw));
    setStatus("unlocked");
  });
  const unlock = submit(async () => {
    await api.vaultUnlock(pw);
    setStatus("unlocked");
    onClose();
  });
  const reset = submit(async () => {
    const bad = checkNew(pw, pw2);
    if (bad) throw bad;
    await api.vaultResetPassword(recovery, pw);
    setStatus("unlocked");
    onClose();
  });
  const change = submit(async () => {
    const bad = checkNew(pw, pw2);
    if (bad) throw bad;
    await api.vaultChangePassword(oldPw, pw);
    setView("main");
    setInfo(t.vault.changed);
  });
  async function lock() {
    await api.vaultLock();
    setStatus("locked");
    onClose();
  }

  async function copyKey() {
    if (!shownKey) return;
    try {
      await navigator.clipboard.writeText(shownKey);
      setCopied(true);
    } catch {
      setError("No se pudo copiar: anótala a mano");
    }
  }

  const cancel = (
    <Button type="button" variant="outline" onClick={onClose}><X data-icon="inline-start" />{t.cancel}</Button>
  );
  const err = error && <p role="alert" className="text-sm text-destructive">{error}</p>;

  // La clave de recuperación solo puede cerrarse tras confirmar que se guardó (no se cierra con clic fuera).
  const locked = shownKey !== null;

  // Si se abre sobre otra ventana (p. ej. el detalle de un pedido), su fondo no repite el desenfoque.
  const over = useRef(document.querySelector("[data-slot=dialog-overlay]") !== null).current;

  return (
    <DepthContext.Provider value={over ? 1 : 0}>
    <Dialog open onOpenChange={(open, ev) => {
      if (open) return;
      if (locked || ev?.reason === "outside-press") return ev?.cancel();
      onClose();
    }}>
      <DialogContent className="sm:max-w-md" showCloseButton={!locked}>
        {shownKey ? (
          <>
            <DialogHeader><DialogTitle>{t.vault.recoveryTitle}</DialogTitle></DialogHeader>
            <p className="text-sm text-muted-foreground">{t.vault.recoveryHint}</p>
            <code className="block break-all rounded-md border bg-muted p-3 text-center font-mono text-base tracking-wide select-all">{shownKey}</code>
            {err}
            <Button variant="outline" onClick={copyKey}>{copied ? <Check data-icon="inline-start" /> : <Copy data-icon="inline-start" />}{copied ? t.vault.copied : t.vault.copy}</Button>
            <label className="flex items-center gap-2 text-sm">
              <input type="checkbox" checked={saved} onChange={(e) => setSaved(e.target.checked)} />
              {t.vault.saved}
            </label>
            <div className="flex justify-end">
              <Button disabled={!saved} onClick={() => { setShownKey(null); onClose(); }}><Check data-icon="inline-start" />{t.vault.done}</Button>
            </div>
          </>
        ) : status === "none" ? (
          <form onSubmit={create} className="space-y-3">
            <DialogHeader><DialogTitle>{t.vault.createTitle}</DialogTitle></DialogHeader>
            <p className="text-sm text-muted-foreground">{t.vault.createHint}</p>
            <PasswordField id="vpw" label={t.vault.password} value={pw} onChange={setPw} autoFocus />
            <PasswordField id="vpw2" label={t.vault.confirm} value={pw2} onChange={setPw2} />
            {err}
            <div className="flex justify-end gap-2">{cancel}<Button type="submit" disabled={busy}><KeyRound data-icon="inline-start" />{t.vault.create}</Button></div>
          </form>
        ) : status === "locked" && view === "recover" ? (
          <form onSubmit={reset} className="space-y-3">
            <DialogHeader><DialogTitle>{t.vault.resetTitle}</DialogTitle></DialogHeader>
            <div className="space-y-1">
              <Label htmlFor="vrec">{t.vault.recoveryKey}</Label>
              <Input id="vrec" autoComplete="off" autoFocus className="font-mono" value={recovery} onChange={(e) => setRecovery(e.target.value)} />
            </div>
            <PasswordField id="vpw" label={t.vault.newPassword} value={pw} onChange={setPw} />
            <PasswordField id="vpw2" label={t.vault.confirm} value={pw2} onChange={setPw2} />
            {err}
            <div className="flex justify-end gap-2">
              <Button type="button" variant="outline" onClick={() => { setView("main"); setError(null); clear(); }}>{t.vault.back}</Button>
              <Button type="submit" disabled={busy}><KeyRound data-icon="inline-start" />{t.vault.reset}</Button>
            </div>
          </form>
        ) : status === "locked" ? (
          <form onSubmit={unlock} className="space-y-3">
            <DialogHeader><DialogTitle>{t.vault.unlockTitle}</DialogTitle></DialogHeader>
            <PasswordField id="vpw" label={t.vault.password} value={pw} onChange={setPw} autoFocus />
            {err}
            <div className="flex items-center justify-between gap-2">
              <Button type="button" variant="link" className="px-0" onClick={() => { setView("recover"); setError(null); clear(); }}>{t.vault.forgot}</Button>
              <div className="flex gap-2">{cancel}<Button type="submit" disabled={busy}><LockOpen data-icon="inline-start" />{t.vault.unlock}</Button></div>
            </div>
          </form>
        ) : view === "change" ? (
          <form onSubmit={change} className="space-y-3">
            <DialogHeader><DialogTitle>{t.vault.changePassword}</DialogTitle></DialogHeader>
            <PasswordField id="vold" label={t.vault.oldPassword} value={oldPw} onChange={setOldPw} autoFocus />
            <PasswordField id="vpw" label={t.vault.newPassword} value={pw} onChange={setPw} />
            <PasswordField id="vpw2" label={t.vault.confirm} value={pw2} onChange={setPw2} />
            {err}
            <div className="flex justify-end gap-2">
              <Button type="button" variant="outline" onClick={() => { setView("main"); setError(null); clear(); }}>{t.vault.back}</Button>
              <Button type="submit" disabled={busy}><Save data-icon="inline-start" />{t.vault.change}</Button>
            </div>
          </form>
        ) : (
          <div className="space-y-3">
            <DialogHeader><DialogTitle>{t.vault.unlockedTitle}</DialogTitle></DialogHeader>
            <p className="text-sm text-muted-foreground">{t.vault.autoLockHint}</p>
            {info && <p className="text-sm">{info}</p>}
            {err}
            <div className="flex flex-wrap justify-end gap-2">
              <Button variant="outline" onClick={() => { setView("change"); setInfo(null); }}><KeyRound data-icon="inline-start" />{t.vault.changePassword}</Button>
              <Button onClick={() => void lock()}><Lock data-icon="inline-start" />{t.vault.lockNow}</Button>
            </div>
          </div>
        )}
      </DialogContent>
    </Dialog>
    </DepthContext.Provider>
  );
}

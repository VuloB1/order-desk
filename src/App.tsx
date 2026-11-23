import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ArrowUpCircle, DatabaseBackup, Lock, LockOpen, Minus, Moon, Square, Sun, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import OrdersPage from "@/features/orders/OrdersPage";
import { api, errorMessage } from "@/lib/api";
import ConfirmDialog from "@/components/ConfirmDialog";
import { t } from "@/i18n/es";
import Backdrop from "@/components/Backdrop";
import Logo from "@/components/Logo";
import BackupDialog from "@/features/backup/BackupDialog";
import BackupReminder from "@/features/backup/BackupReminder";
import { VaultProvider, useVault } from "@/features/vault/VaultContext";

function VaultButton() {
  const { status, openDialog } = useVault();
  const label = t.vault.openLabel(status);
  return (
    <Button variant="ghost" size="icon-sm" aria-label={label} title={label} onClick={openDialog}>
      {status === "unlocked" ? <LockOpen /> : <Lock />}
    </Button>
  );
}

function BackupButton() {
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button variant="ghost" size="icon-sm" aria-label={t.backup.open} title={t.backup.open} onClick={() => setOpen(true)}>
        <DatabaseBackup />
      </Button>
      {open && <BackupDialog onClose={() => setOpen(false)} />}
    </>
  );
}

function UpdateButton() {
  const [update, setUpdate] = useState<{ version: string } | null>(null);
  const [confirm, setConfirm] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Si no hay conexión simplemente no aparece nada: la app funciona igual sin internet.
  useEffect(() => {
    api.checkUpdate().then(setUpdate).catch(() => {});
  }, []);
  if (!update) return null;
  const label = t.update.available(update.version);
  return (
    <>
      <Button variant="ghost" size="icon-sm" aria-label={label} title={label} className="text-emerald-500" onClick={() => setConfirm(true)}>
        <ArrowUpCircle />
      </Button>
      <ConfirmDialog
        open={confirm}
        onOpenChange={setConfirm}
        title={t.update.title(update.version)}
        body={error ? t.update.failed + error : t.update.body}
        confirmLabel={t.update.install}
        onConfirm={() => {
          setError(null);
          api.installUpdate().catch((e) => { setError(errorMessage(e)); setConfirm(true); });
        }}
      />
    </>
  );
}

export default function App() {
  const [version, setVersion] = useState("");
  const [dark, setDark] = useState(() => window.matchMedia("(prefers-color-scheme: dark)").matches);
  // Hueco de la cabecera donde las pantallas dibujan sus accesos (ver OrdersPage).
  const [slot, setSlot] = useState<HTMLElement | null>(null);

  useEffect(() => {
    api.version().then(setVersion);
  }, []);
  useEffect(() => {
    document.documentElement.classList.toggle("dark", dark);
  }, [dark]);

  function toggleTheme() {
    // La clase solo existe durante el cambio: así la transición no afecta al resto de la app.
    const root = document.documentElement;
    root.classList.add("theme-transition");
    setDark((d) => !d);
    window.setTimeout(() => root.classList.remove("theme-transition"), 500);
  }

  return (
    <VaultProvider>
    <div className="flex h-screen flex-col text-foreground">
      <Backdrop />
      {/* Misma anchura (max-w-6xl) y margen que el contenido, para que cabecera y página se alineen. */}
      {/* Barra de título propia (la ventana no tiene decoración de Windows): arrastrable y con sus controles. */}
      <header data-tauri-drag-region className="z-10 flex shrink-0 items-stretch border-b bg-card">
        <div data-tauri-drag-region className="mx-auto flex max-w-6xl h-9 flex-1 items-center gap-3 px-6">
          <h1 className="pointer-events-none flex items-center gap-2 text-base font-semibold"><Logo className="size-5" />Order Desk</h1>
          <span className="pointer-events-none rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground">v{version}</span>
          {/* Todos los iconos en un solo grupo para que el espacio entre ellos sea igual. */}
          <div className="ml-auto flex items-center gap-1">
            <div ref={setSlot} className="contents" />
            <UpdateButton />
            <BackupButton />
            <VaultButton />
            <Button variant="ghost" size="icon-sm" aria-label={t.theme(dark)} title={t.theme(dark)} onClick={toggleTheme}>
              {dark ? <Sun key="sun" className="theme-icon" /> : <Moon key="moon" className="theme-icon" />}
            </Button>
          </div>
        </div>
        <div className="flex shrink-0 self-start">
          {[
            { label: "Minimizar", icon: Minus, run: () => getCurrentWindow().minimize(), cls: "hover:bg-muted" },
            { label: "Maximizar", icon: Square, run: () => getCurrentWindow().toggleMaximize(), cls: "hover:bg-muted" },
            { label: "Cerrar", icon: X, run: () => getCurrentWindow().close(), cls: "hover:bg-red-600 hover:text-white" },
          ].map(({ label, icon: Icon, run, cls }) => (
            <button
              key={label}
              type="button"
              aria-label={label}
              title={label}
              onClick={() => void run()}
              className={`flex h-9 w-12 items-center justify-center text-muted-foreground transition-colors ${cls}`}
            >
              <Icon className="size-4" />
            </button>
          ))}
        </div>
      </header>
      {/* Solo el contenido se desplaza: la barra de título queda fija y la barra de desplazamiento empieza debajo. */}
      <div className="min-h-0 flex-1 overflow-y-auto">
        <BackupReminder />
        <OrdersPage actionsSlot={slot} />
      </div>
    </div>
    </VaultProvider>
  );
}

import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, type VaultStatus } from "@/lib/api";
import VaultDialog from "./VaultDialog";

interface Vault {
  status: VaultStatus;
  /** Abre el diálogo de la sección privada (crear / desbloquear / gestionar). */
  openDialog: () => void;
  setStatus: (s: VaultStatus) => void;
}

const Ctx = createContext<Vault>({ status: "none", openDialog: () => {}, setStatus: () => {} });
export const useVault = () => useContext(Ctx);

export function VaultProvider({ children }: { children: ReactNode }) {
  const [status, setStatus] = useState<VaultStatus>("none");
  const [open, setOpen] = useState(false);

  useEffect(() => {
    api.vaultStatus().then(setStatus).catch(() => setStatus("none"));
    // El backend ya destruyó la clave: aquí solo se vacía el estado de la interfaz.
    const un = listen("private-lock", () => setStatus("locked"));
    return () => void un.then((f) => f());
  }, []);

  const openDialog = useCallback(() => setOpen(true), []);
  const value = useMemo(() => ({ status, openDialog, setStatus }), [status, openDialog]);

  return (
    <Ctx.Provider value={value}>
      {children}
      {open && <VaultDialog status={status} setStatus={setStatus} onClose={() => setOpen(false)} />}
    </Ctx.Provider>
  );
}

const KEY = "orderdesk.lastBackup";
const REMIND_AFTER_DAYS = 30;

export function lastBackup(): number | null {
  try {
    const v = Number(localStorage.getItem(KEY));
    return v > 0 ? v : null;
  } catch {
    return null;
  }
}

/** Se llama al crear una copia: apunta la fecha y avisa a los recordatorios abiertos. */
export function markBackup() {
  try {
    localStorage.setItem(KEY, String(Date.now()));
  } catch { /* sin almacenamiento solo se pierde el recordatorio */ }
  window.dispatchEvent(new Event("backup-done"));
}

export const backupIsStale = (last: number | null, now = Date.now()) =>
  last === null || now - last > REMIND_AFTER_DAYS * 86_400_000;

import type { ReactNode } from "react";

/** Línea de solo lectura "etiqueta: valor" para las fichas de datos guardados. */
export default function InfoRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex gap-2 text-sm">
      <span className="w-44 shrink-0 text-muted-foreground">{label}</span>
      <span className="min-w-0 break-words">{children}</span>
    </div>
  );
}

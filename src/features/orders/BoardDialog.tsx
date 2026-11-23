import { useEffect, useState } from "react";
import { Save } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { api, errorMessage } from "@/lib/api";
import { t } from "@/i18n/es";

/** Dónde se publica el tablero: dirección https:// del repositorio de GitHub (con Pages activado). */
export default function BoardDialog({ onClose, onSaved }: { onClose: () => void; onSaved: (configured: boolean) => void }) {
  const [url, setUrl] = useState("");
  const [error, setError] = useState<string | null>(null);
  const x = t.board;

  useEffect(() => {
    api.getBoardSettings().then((s) => setUrl(s.repoUrl)).catch((e) => setError(errorMessage(e)));
  }, []);

  async function save() {
    setError(null);
    try {
      const s = await api.setBoardSettings(url);
      onSaved(s.repoUrl !== "");
      onClose();
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader><DialogTitle>{x.settingsTitle}</DialogTitle></DialogHeader>
        <form className="space-y-4 text-sm" onSubmit={(e) => { e.preventDefault(); void save(); }}>
          <p className="text-muted-foreground">{x.settingsIntro}</p>
          <div className="space-y-1.5">
            <Label htmlFor="board-url">{x.repoLabel}</Label>
            <Input id="board-url" value={url} placeholder="https://github.com/usuario/pedidos" onChange={(e) => setUrl(e.target.value)} autoFocus />
          </div>
          {error && <p role="alert" className="text-destructive">{error}</p>}
          <div className="flex justify-end gap-2">
            <Button type="button" variant="ghost" onClick={onClose}>{t.cancel}</Button>
            <Button type="submit"><Save data-icon="inline-start" />{x.save}</Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}

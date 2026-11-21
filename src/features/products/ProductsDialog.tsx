import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { GripVertical, Pencil, Plus, Save, Trash2, X } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import ConfirmDialog from "@/components/ConfirmDialog";
import { api, errorMessage, type Product } from "@/lib/api";
import { centsToInput, formatUsd, parseUsd } from "@/lib/money";
import { t } from "@/i18n/es";

interface Props {
  products: Product[];
  onClose: () => void;
  onChanged: () => void; // recargar el catálogo en la página
}

interface Draft {
  id: string | null; // null = producto nuevo
  name: string;
  price: string;
  active: boolean;
}

const blank: Draft = { id: null, name: "", price: "", active: true };

export default function ProductsDialog({ products, onClose, onChanged }: Props) {
  const [d, setD] = useState<Draft | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [drag, setDrag] = useState<string | null>(null); // id del producto que se arrastra
  const [order, setOrder] = useState<string[] | null>(null); // orden provisional mientras se arrastra
  const rows = useRef(new Map<string, HTMLLIElement>());
  const tops = useRef(new Map<string, number>());
  const ulRef = useRef<HTMLUListElement>(null);
  const start = useRef({ y: 0, top: 0, scroll: 0 }); // puntero y posición de la fila al empezar a arrastrar
  const pointerY = useRef(0);
  const latest = useRef({ order, products });
  latest.current = { order, products };

  const list = order ? order.flatMap((id) => products.filter((p) => p.id === id)) : products;

  // Cuando el orden cambia, el resto de filas se deslizan desde donde estaban (FLIP).
  useLayoutEffect(() => {
    const calm = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    rows.current.forEach((el, id) => {
      if (id === drag) {
        tops.current.set(id, el.offsetTop);
        return follow(el);
      }
      const now = el.offsetTop; // offsetTop ignora transform: no se confunde con una animación en curso
      const before = tops.current.get(id);
      if (!calm && before !== undefined && before !== now) {
        el.style.transition = "none";
        el.style.transform = `translateY(${before - now}px)`;
        void el.offsetHeight;
        el.style.transition = "transform 180ms ease";
        el.style.transform = "";
      }
      tops.current.set(id, now);
    });
  });
  /** La fila arrastrada sigue al puntero, sin salirse de la parte visible de la lista. */
  function follow(el: HTMLLIElement) {
    const ul = ulRef.current;
    if (!ul) return;
    let dy = pointerY.current - start.current.y + (start.current.top - el.offsetTop) + (ul.scrollTop - start.current.scroll);
    const min = ul.scrollTop - el.offsetTop;
    dy = Math.min(Math.max(dy, min), min + ul.clientHeight - el.offsetHeight);
    el.style.transition = "none";
    el.style.transform = `translateY(${dy}px)`;
  }

  // Arrastre con eventos de puntero: el cursor se controla por completo (manita agarrando).
  useEffect(() => {
    if (!drag) return;
    const grabbing = document.createElement("style");
    grabbing.textContent = "*{cursor:grabbing!important;user-select:none!important}";
    document.head.append(grabbing);

    const update = () => {
      const ul = ulRef.current;
      const dragged = rows.current.get(drag);
      if (!ul || !dragged) return;
      follow(dragged);
      // La fila destino se decide por la posición de diseño (offsetTop, sin transform): la animación no provoca bucles.
      const y = pointerY.current - ul.getBoundingClientRect().top - ul.clientTop + ul.scrollTop;
      setOrder((prev) => {
        const ids = prev ?? latest.current.products.map((p) => p.id);
        let at = ids.findIndex((id) => {
          const el = rows.current.get(id);
          return el !== undefined && y < el.offsetTop + el.offsetHeight;
        });
        if (at < 0) at = ids.length - 1;
        if (ids[at] === drag) return prev;
        const next = ids.filter((id) => id !== drag);
        next.splice(at, 0, drag);
        return next;
      });
    };
    const move = (e: PointerEvent) => {
      pointerY.current = e.clientY;
      update();
    };
    // Desplazamiento automático al acercar el puntero al borde de la lista (más rápido cuanto más cerca).
    const tick = () => {
      const ul = ulRef.current;
      if (ul) {
        const r = ul.getBoundingClientRect();
        const edge = 48;
        const dy = pointerY.current < r.top + edge
          ? -Math.min(1, (r.top + edge - pointerY.current) / edge)
          : pointerY.current > r.bottom - edge
            ? Math.min(1, (pointerY.current - (r.bottom - edge)) / edge)
            : 0;
        const before = ul.scrollTop;
        if (dy) ul.scrollTop += dy * 16;
        if (ul.scrollTop !== before) update();
      }
    };
    const timer = window.setInterval(tick, 16);
    const end = (commit: boolean) => {
      const el = rows.current.get(drag);
      if (el) { el.style.transition = "transform 180ms ease"; el.style.transform = ""; }
      setDrag(null);
      if (commit) void commitOrder(latest.current.order);
      else setOrder(null);
    };
    const up = () => end(true);
    const cancel = () => end(false);
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", cancel);
    return () => {
      window.clearInterval(timer);
      grabbing.remove();
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", cancel);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [drag]);

  // Al llegar el catálogo ya guardado, se descarta el orden provisional.
  useEffect(() => setOrder(null), [products]);

  async function remove() {
    if (!d?.id) return;
    setConfirmDelete(false);
    try {
      await api.deleteProduct(d.id);
      setD(null);
      setError(null);
      onChanged();
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  async function save() {
    if (!d) return;
    const price = parseUsd(d.price);
    if (!d.name.trim()) return setError(t.productNameRequired);
    if (price == null) return setError(t.priceInvalid);
    try {
      if (d.id) await api.updateProduct({ id: d.id, name: d.name, unitPriceCents: price, active: d.active });
      else await api.createProduct(d.name, price);
      setD(null);
      setError(null);
      onChanged();
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  /** Guarda el orden provisional; los botones rápidos lo siguen. */
  async function commitOrder(ids: string[] | null) {
    if (!ids || ids.every((id, i) => id === latest.current.products[i]?.id)) return setOrder(null);
    try {
      await api.reorderProducts(ids);
      setError(null);
      onChanged();
    } catch (e) {
      setOrder(null);
      setError(errorMessage(e));
    }
  }

  const edit = (p: Product) =>
    setD({ id: p.id, name: p.name, price: centsToInput(p.unitPriceCents), active: p.active });

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>{t.products}</DialogTitle>
        </DialogHeader>
        <p className="text-sm text-muted-foreground">{t.productsHint}</p>

        <ul ref={ulRef} className="relative max-h-[50vh] divide-y overflow-y-auto rounded-md border">
          {list.map((p) => (
            <li
              key={p.id}
              ref={(el) => { if (el) rows.current.set(p.id, el); else rows.current.delete(p.id); }}
              className={`row-hover relative flex items-center gap-3 bg-popover px-3 py-2 text-sm ${drag === p.id ? "z-10 bg-accent shadow-md" : ""}`}
            >
              <GripVertical
                aria-label={`${t.drag}: ${p.name}`}
                className="size-4 shrink-0 cursor-grab touch-none text-muted-foreground"
                onPointerDown={(e) => {
                  if (e.button !== 0) return;
                  const row = rows.current.get(p.id);
                  if (!row) return;
                  pointerY.current = e.clientY;
                  start.current = { y: e.clientY, top: row.offsetTop, scroll: ulRef.current?.scrollTop ?? 0 };
                  setDrag(p.id);
                }}
              />
              <span className={p.active ? "font-medium" : "text-muted-foreground line-through"}>{p.name}</span>
              {!p.active && <Badge variant="secondary">{t.inactive}</Badge>}
              <span className="ml-auto">{formatUsd(p.unitPriceCents)}</span>
              <Button variant="ghost" size="sm" onClick={() => edit(p)}><Pencil data-icon="inline-start" />{t.edit}</Button>
            </li>
          ))}
          {products.length === 0 && <li className="px-3 py-4 text-center text-sm text-muted-foreground">{t.noProducts}</li>}
        </ul>

        <Button className="self-start" onClick={() => setD(blank)}><Plus data-icon="inline-start" />{t.newProduct}</Button>

        {error && !d && <p role="alert" className="text-sm text-destructive">{error}</p>}

        <Dialog open={d !== null} onOpenChange={(o) => { if (!o) { setD(null); setError(null); } }}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>{d?.id ? t.edit : t.newProduct}</DialogTitle>
          </DialogHeader>
          {d && (
            <div className="space-y-3">
              <div className="space-y-1">
                <Label htmlFor="pname">{t.productName}</Label>
                <Input id="pname" autoFocus value={d.name} onChange={(e) => setD({ ...d, name: e.target.value })} />
              </div>
              <div className="grid grid-cols-1 gap-2">
                <div className="space-y-1">
                  <Label htmlFor="pprice">{t.priceUsd}</Label>
                  <Input id="pprice" inputMode="decimal" value={d.price} onChange={(e) => setD({ ...d, price: e.target.value })} />
                </div>
              </div>
              {d.id && (
                <label className="flex items-center gap-2 text-sm">
                  <input type="checkbox" checked={d.active} onChange={(e) => setD({ ...d, active: e.target.checked })} />
                  {t.activeHint}
                </label>
              )}
              {error && <p role="alert" className="text-sm text-destructive">{error}</p>}
              <div className="flex items-center gap-2">
                {d.id && <Button variant="destructive" onClick={() => setConfirmDelete(true)}><Trash2 data-icon="inline-start" />{t.deleteProduct}</Button>}
                <div className="ml-auto flex gap-2">
                  <Button variant="outline" onClick={() => { setD(null); setError(null); }}><X data-icon="inline-start" />{t.cancel}</Button>
                  <Button onClick={save}><Save data-icon="inline-start" />{t.save}</Button>
                </div>
              </div>
            </div>
          )}
          <ConfirmDialog
            open={confirmDelete}
            onOpenChange={setConfirmDelete}
            title={t.deleteProductTitle}
            body={t.deleteProductBody}
            confirmLabel={t.deleteProduct}
            onConfirm={() => void remove()}
          />
        </DialogContent>
        </Dialog>
      </DialogContent>
    </Dialog>
  );
}

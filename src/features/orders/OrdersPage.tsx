import { useCallback, useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { api, errorMessage, STATUSES, type Destination, type OrderDetail, type OrderFilter, type OrderStatus, type DiscountCode, type OrderSummary, type Product } from "@/lib/api";
import { todayIso } from "@/lib/money";
import { createPortal } from "react-dom";
import { BarChart3, Download, FileJson, FileSpreadsheet, Package, Plus, Ticket } from "lucide-react";
import { destinationItems, statusItems, statusLabel, t } from "@/i18n/es";
import DiscountCodesDialog from "@/features/discounts/DiscountCodesDialog";
import ProductsDialog from "@/features/products/ProductsDialog";
import StatsDialog from "@/features/stats/StatsDialog";
import { useVault } from "@/features/vault/VaultContext";
import OrderDetailDialog from "./OrderDetailDialog";
import OrderForm from "./OrderForm";
import OrderRow from "./OrderRow";

const ALL = "all"; // Select no admite valor vacío

export default function OrdersPage({ actionsSlot }: { actionsSlot: HTMLElement | null }) {
  const [orders, setOrders] = useState<OrderSummary[]>([]);
  const [status, setStatus] = useState(ALL);
  const [destination, setDestination] = useState(ALL);
  const [search, setSearch] = useState("");
  const [query, setQuery] = useState(""); // `search` con 200 ms de retraso
  const [error, setError] = useState<string | null>(null);
  const [detailId, setDetailId] = useState<string | null>(null);
  const [form, setForm] = useState<{ editing?: OrderDetail } | null>(null);
  const [products, setProducts] = useState<Product[]>([]);
  const [productsDialog, setProductsDialog] = useState(false);
  const [codes, setCodes] = useState<DiscountCode[]>([]);
  const [codesDialog, setCodesDialog] = useState(false);
  const [statsDialog, setStatsDialog] = useState(false);
  const { status: vault } = useVault(); // al (des)bloquear cambia lo que busca la búsqueda
  const [exportOpen, setExportOpen] = useState(false);
  const [exportMsg, setExportMsg] = useState<string | null>(null);

  const filter: OrderFilter = {
    status: status === ALL ? undefined : (status as OrderStatus),
    destination: destination === ALL ? undefined : (destination as Destination),
    search: query || undefined,
  };

  async function exportOrders(format: "csv" | "json") {
    setExportOpen(false);
    setExportMsg(null);
    try {
      const r = await api.exportOrders(format, filter, `pedidos-${todayIso()}.${format}`);
      if (r) setExportMsg(t.exported(r.count, r.path));
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  const reload = useCallback(async () => {
    try {
      setOrders(
        await api.listOrders({
          status: status === ALL ? undefined : (status as OrderStatus),
          destination: destination === ALL ? undefined : (destination as Destination),
          search: query || undefined,
        }),
      );
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    }
  }, [status, destination, query, vault]);

  useEffect(() => {
    const id = window.setTimeout(() => setQuery(search), 200);
    return () => window.clearTimeout(id);
  }, [search]);
  useEffect(() => void reload(), [reload]);
  const loadProducts = useCallback(
    () => api.listProducts().then(setProducts).catch((e) => setError(errorMessage(e))),
    [],
  );
  const loadCodes = useCallback(
    () => api.listDiscountCodes().then(setCodes).catch((e) => setError(errorMessage(e))),
    [],
  );
  useEffect(() => {
    void loadProducts();
    void loadCodes();
  }, [loadProducts, loadCodes]);

  return (
    <div className="mx-auto max-w-6xl space-y-4 p-6">
      <div className="flex flex-wrap items-center gap-2">
        <Input className="w-56" aria-label={t.search} placeholder={vault === "unlocked" ? t.searchPrivate : t.search} value={search} onChange={(e) => setSearch(e.target.value)} />
        <Select
          value={destination}
          items={[{ value: ALL, label: t.allDestinations }, ...destinationItems]}
          onValueChange={(v) => setDestination(v ?? ALL)}
        >
          <SelectTrigger className="w-56"><SelectValue /></SelectTrigger>
          <SelectContent>
            <SelectItem value={ALL}>{t.allDestinations}</SelectItem>
            {destinationItems.map((o) => <SelectItem key={o.value} value={o.value}>{o.label}</SelectItem>)}
          </SelectContent>
        </Select>
        <Select
          value={status}
          items={[{ value: ALL, label: t.allStatuses }, ...statusItems]}
          onValueChange={(v) => setStatus(v ?? ALL)}
        >
          <SelectTrigger className="w-56"><SelectValue /></SelectTrigger>
          <SelectContent>
            <SelectItem value={ALL}>{t.allStatuses}</SelectItem>
            {STATUSES.map((s) => <SelectItem key={s} value={s}>{statusLabel[s]}</SelectItem>)}
          </SelectContent>
        </Select>
        <Button className="ml-auto" onClick={() => setForm({})}><Plus data-icon="inline-start" />{t.newOrder}</Button>
      </div>

      {/* Los accesos a catálogos viven en la cabecera de App; se dibujan en su hueco. */}
      {actionsSlot &&
        createPortal(
          <>
            <Popover open={exportOpen} onOpenChange={setExportOpen}>
              <PopoverTrigger render={<Button variant="ghost" size="icon-sm" aria-label={t.exportLabel} title={t.exportLabel} />}>
                <Download />
              </PopoverTrigger>
              <PopoverContent align="end" className="w-64 space-y-2 p-2">
                <p className="px-1 text-xs text-muted-foreground">{t.exportOnlyPublic}</p>
                <Button variant="outline" className="w-full justify-start" onClick={() => void exportOrders("csv")}><FileSpreadsheet data-icon="inline-start" />{t.exportCsv}</Button>
                <Button variant="outline" className="w-full justify-start" onClick={() => void exportOrders("json")}><FileJson data-icon="inline-start" />{t.exportJson}</Button>
              </PopoverContent>
            </Popover>
            <Button variant="ghost" size="icon-sm" aria-label={t.stats.open} title={t.stats.open} onClick={() => setStatsDialog(true)}><BarChart3 /></Button>
            <Button variant="ghost" size="icon-sm" aria-label={t.products} title={t.products} onClick={() => setProductsDialog(true)}><Package /></Button>
            <Button variant="ghost" size="icon-sm" aria-label={t.codesTitle} title={t.codesTitle} onClick={() => setCodesDialog(true)}><Ticket /></Button>
          </>,
          actionsSlot,
        )}

      {exportMsg && <p role="status" className="text-sm break-all">{exportMsg}</p>}
      {error && <p role="alert" className="text-sm text-destructive">{error}</p>}

      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>{t.number}</TableHead>
            <TableHead>{t.date}</TableHead>
            <TableHead>{t.destination}</TableHead>
            <TableHead>{t.status}</TableHead>
            <TableHead>{t.invoice}</TableHead>
            <TableHead className="text-right">{t.items}</TableHead>
            <TableHead className="text-right">{t.total}</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {orders.map((o) => <OrderRow key={o.id} order={o} onOpen={setDetailId} />)}
          {orders.length === 0 && (
            <TableRow><TableCell colSpan={7} className="py-8 text-center text-muted-foreground">{t.empty}</TableCell></TableRow>
          )}
        </TableBody>
      </Table>

      {detailId && (
        <OrderDetailDialog
          id={detailId}
          onClose={() => setDetailId(null)}
          onChanged={reload}
          onEdit={(o) => { setDetailId(null); setForm({ editing: o }); }}
        />
      )}
      {form && (
        <OrderForm
          products={products}
          codes={codes}
          editing={form.editing}
          onClose={() => setForm(null)}
          onSaved={(o) => { setForm(null); setDetailId(o.id); void reload(); }}
        />
      )}
      {productsDialog && (
        <ProductsDialog products={products} onClose={() => setProductsDialog(false)} onChanged={loadProducts} />
      )}
      {statsDialog && <StatsDialog onClose={() => setStatsDialog(false)} />}
      {codesDialog && <DiscountCodesDialog codes={codes} onClose={() => setCodesDialog(false)} onChanged={loadCodes} />}
    </div>
  );
}

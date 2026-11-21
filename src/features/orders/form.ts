import type { Destination, DiscountCode, OrderDetail, OrderInput, OrderStatus, Product } from "@/lib/api";
import { centsToInput, parseUsd, todayIso } from "@/lib/money";

export interface LineDraft {
  productId: string | null; // null = producto personalizado
  name: string;
  quantity: string;
  price: string; // USD
}

export interface OrderDraft {
  externalNumber: string;
  orderDate: string;
  status: OrderStatus;
  destination: Destination;
  invoiceRequired: boolean;
  invoiceNumber: string; // con número se considera que lleva factura
  shipping: string; // USD
  code: DiscountCode | null; // si hay código, el descuento se calcula; si no, vale `discount`
  discount: string; // USD, manual
  items: LineDraft[];
}

export const emptyLine = (): LineDraft => ({ productId: null, name: "", quantity: "1", price: "" });

export const MAX_QUICK_QTY = 20;

const isBlank = (l: LineDraft) => !l.productId && !l.name.trim() && !l.price.trim();

/**
 * Un toque en un preset: suma 1 a su línea si ya existe, o añade una con el precio del catálogo.
 * Una línea en blanco (la inicial) se reutiliza en vez de dejarla vacía.
 */
export function addPreset(items: LineDraft[], p: Product): LineDraft[] {
  if (items.some((l) => l.productId === p.id)) {
    return items.map((l) => (l.productId === p.id ? { ...l, quantity: String((Number(l.quantity) || 0) + 1) } : l));
  }
  const line: LineDraft = { productId: p.id, name: p.name, quantity: "1", price: centsToInput(p.unitPriceCents) };
  return [...items.filter((l) => !isBlank(l)), line];
}

/** Importe de una línea en céntimos (cantidad × precio); 0 mientras algún campo sea inválido. */
export function lineTotal(l: LineDraft): number {
  const q = /^\d+$/.test(l.quantity.trim()) ? Number(l.quantity) : 0;
  return q * (parseUsd(l.price) ?? 0);
}

/** Descuento de un código sobre el subtotal de productos (sin envío), en céntimos. */
export function discountFor(code: DiscountCode, subtotal: number): number {
  const raw = code.kind === "percent" ? Math.round((subtotal * code.value) / 100) : code.value;
  return Math.min(raw, subtotal);
}

export const emptyDraft = (): OrderDraft => ({
  externalNumber: "",
  orderDate: todayIso(),
  status: "pending",
  destination: "chile", // la tienda opera desde Chile
  invoiceRequired: false,
  invoiceNumber: "",
  shipping: "",
  code: null,
  discount: "",
  items: [emptyLine()],
});

/**
 * Pedido guardado → borrador. El código solo se reaplica si da exactamente el descuento guardado;
 * si el código cambió desde entonces, se conserva el importe como descuento manual.
 */
export function detailToDraft(o: OrderDetail, codes: DiscountCode[] = []): OrderDraft {
  const items = o.items.map((i) => ({
    productId: i.productId,
    name: i.name,
    quantity: String(i.quantity),
    price: centsToInput(i.unitPriceCents),
  }));
  const found = codes.find((c) => c.code.toLowerCase() === o.discountCode?.toLowerCase());
  const code = found && discountFor(found, o.subtotalCents) === o.discountCents ? found : null;
  return {
    externalNumber: o.externalNumber,
    orderDate: o.orderDate,
    status: o.status,
    destination: o.destination,
    invoiceRequired: !!o.invoiceRequired,
    invoiceNumber: o.invoiceNumber ?? "",
    shipping: o.shippingCostCents ? centsToInput(o.shippingCostCents) : "",
    code,
    discount: !code && o.discountCents ? centsToInput(o.discountCents) : "",
    items,
  };
}

/** Totales en vivo; los campos aún inválidos o vacíos cuentan como 0. */
export function draftTotals(d: OrderDraft) {
  const subtotal = d.items.reduce((a, l) => a + lineTotal(l), 0);
  const discount = d.code ? discountFor(d.code, subtotal) : (parseUsd(d.discount) ?? 0);
  return { subtotal, discount, total: subtotal + (parseUsd(d.shipping) ?? 0) - discount };
}

export type DraftResult = { ok: true; input: OrderInput } | { ok: false; error: string };

/** Convierte el formulario a la entrada del backend; el backend vuelve a validar todo. */
export function draftToInput(d: OrderDraft): DraftResult {
  const fail = (error: string): DraftResult => ({ ok: false, error });
  if (!d.externalNumber.trim()) return fail("Escribe el número del pedido");
  if (!d.orderDate) return fail("Elige la fecha del pedido");

  const items: OrderInput["items"] = [];
  for (const [n, l] of d.items.entries()) {
    const quantity = /^\d+$/.test(l.quantity.trim()) ? Number(l.quantity) : 0;
    const unitPriceCents = parseUsd(l.price);
    if (!l.name.trim()) return fail(`Producto ${n + 1}: falta el nombre`);
    if (quantity < 1) return fail(`Producto ${n + 1}: la cantidad debe ser un entero mayor que 0`);
    if (unitPriceCents == null) return fail(`Producto ${n + 1}: precio en USD no válido`);
    items.push({ productId: l.productId, name: l.name, quantity, unitPriceCents });
  }

  const optionalUsd = (s: string) => (s.trim() === "" ? 0 : parseUsd(s));
  const shippingCostCents = optionalUsd(d.shipping);
  if (shippingCostCents == null) return fail("Gastos de envío no válidos");
  const manual = optionalUsd(d.discount);
  if (!d.code && manual == null) return fail("Descuento no válido");

  return {
    ok: true,
    input: {
      externalNumber: d.externalNumber,
      orderDate: d.orderDate,
      status: d.status,
      destination: d.destination,
      invoiceRequired: d.invoiceRequired || !!d.invoiceNumber.trim(),
      invoiceNumber: d.invoiceNumber.trim() || null,
      shippingCostCents,
      discountCents: draftTotals(d).discount,
      discountCode: d.code?.code ?? null,
      items,
    },
  };
}

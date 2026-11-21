import { expect, test } from "vitest";
import type { DiscountCode, OrderDetail, Product } from "@/lib/api";
import { addPreset, detailToDraft, discountFor, draftToInput, draftTotals, emptyDraft, lineTotal, type OrderDraft } from "./form";

const line = (name: string, quantity: string, price: string) => ({ productId: null, name, quantity, price });

const base = (over: Partial<OrderDraft> = {}): OrderDraft => ({
  ...emptyDraft(),
  externalNumber: "#1424",
  orderDate: "2026-03-10",
  shipping: "5",
  items: [line("Tracker de prueba", "2", "25"), line("Extensión de prueba", "1", "10,00")],
  ...over,
});

const preset = (id: string, name: string, unitPriceCents: number): Product => ({
  id,

  name,
  unitPriceCents,
  active: true,
});

const code = (c: string, kind: DiscountCode["kind"], value: number): DiscountCode => ({
  id: c,
  code: c,
  kind,
  value,
  active: true,
});

test("totales en vivo en USD", () => {
  expect(draftTotals(base())).toEqual({ subtotal: 6000, discount: 0, total: 6500 });
  expect(draftTotals(base({ discount: "5" }))).toEqual({ subtotal: 6000, discount: 500, total: 6000 });
});

test("totales toleran campos incompletos", () => {
  const d = base({ items: [line("", "x", "")], shipping: "abc" });
  expect(draftTotals(d)).toEqual({ subtotal: 0, discount: 0, total: 0 });
});

test("convierte el formulario a la entrada del backend conservando el número tal cual", () => {
  const r = draftToInput(base());
  expect(r.ok).toBe(true);
  if (!r.ok) return;
  expect(r.input).toMatchObject({
    externalNumber: "#1424",
    shippingCostCents: 500,
    discountCents: 0,
    discountCode: null,
  });
  expect(r.input.items).toEqual([
    { productId: null, name: "Tracker de prueba", quantity: 2, unitPriceCents: 2500 },
    { productId: null, name: "Extensión de prueba", quantity: 1, unitPriceCents: 1000 },
  ]);
});

test("el destino es Chile por defecto y se envía y recupera al editar", () => {
  expect(emptyDraft().destination).toBe("chile");
  const r = draftToInput(base({ destination: "foreign" }));
  expect(r.ok && r.input.destination).toBe("foreign");
  expect(detailToDraft(saved({ destination: "foreign" })).destination).toBe("foreign");
});

test("rechaza formularios inválidos con un mensaje", () => {
  const cases: Partial<OrderDraft>[] = [
    { externalNumber: "  " },
    { shipping: "1.234" },
    { discount: "abc" },
    { items: [line("A", "0", "1")] },
    { items: [line("A", "1", "x")] },
    { items: [line(" ", "1", "1")] },
  ];
  for (const c of cases) expect(draftToInput(base(c)).ok).toBe(false);
});

test("un preset reemplaza la línea en blanco inicial y rellena nombre y precio", () => {
  const items = addPreset(emptyDraft().items, preset("p1", "Producto de prueba", 9900));
  expect(items).toEqual([{ productId: "p1", name: "Producto de prueba", quantity: "1", price: "99.00" }]);
});

test("tocar el mismo preset de nuevo suma 1 y otro preset añade una línea", () => {
  const a = preset("p1", "A de prueba", 1000);
  const b = preset("p2", "B de prueba", 250);
  let items = addPreset(addPreset(emptyDraft().items, a), a);
  expect(items.map((l) => [l.productId, l.quantity])).toEqual([["p1", "2"]]);
  items = addPreset(items, b);
  expect(items.map((l) => l.productId)).toEqual(["p1", "p2"]);
});

test("el preset conserva las líneas ya escritas y llega al backend con su productId", () => {
  const items = addPreset([line("Personalizado", "1", "5")], preset("p1", "A", 1000));
  const r = draftToInput(base({ items }));
  expect(r.ok && r.input.items.map((i) => i.productId)).toEqual([null, "p1"]);
});

test("el importe de la línea y los totales siguen a la cantidad", () => {
  const [p] = addPreset([], preset("p1", "A de prueba", 9900));
  for (const [q, cents] of [["1", 9900], ["3", 29700], ["20", 198000]] as const) {
    const l = { ...p, quantity: q };
    expect(lineTotal(l)).toBe(cents);
    expect(draftTotals(base({ items: [l], shipping: "" })).total).toBe(cents);
  }
  expect(lineTotal({ ...p, quantity: "" })).toBe(0);
});

test("descuento por código: porcentaje redondeado y fijo con tope en el subtotal", () => {
  expect(discountFor(code("P10", "percent", 10), 6000)).toBe(600);
  expect(discountFor(code("P15", "percent", 15), 999)).toBe(150); // 149,85 → 150
  expect(discountFor(code("F5", "fixed", 500), 6000)).toBe(500);
  expect(discountFor(code("F50", "fixed", 5000), 1200)).toBe(1200); // nunca más que el subtotal
});

test("con código aplicado el descuento se calcula solo y se envía con su código", () => {
  const d = base({ code: code("VERANO10", "percent", 10), discount: "99" }); // el manual se ignora
  expect(draftTotals(d)).toEqual({ subtotal: 6000, discount: 600, total: 5900 });
  const r = draftToInput(d);
  expect(r.ok && [r.input.discountCents, r.input.discountCode]).toEqual([600, "VERANO10"]);
  // si cambian las líneas, el porcentaje sigue al subtotal
  const d2 = { ...d, items: [line("A", "1", "10")] };
  expect(draftTotals(d2).discount).toBe(100);
});

const saved = (over: Partial<OrderDetail> = {}) =>
  ({
    storeId: "s",
    externalNumber: "#1",
    orderDate: "2026-03-10",
    status: "pending",
    shippingCostCents: 0,
    subtotalCents: 6000,
    discountCents: 600,
    discountCode: "verano10",
    items: [{ productId: null, name: "A", quantity: 1, unitPriceCents: 6000 }],
    ...over,
  }) as OrderDetail;

test("al editar, el código se reaplica solo si da el mismo descuento guardado", () => {
  const codes = [code("VERANO10", "percent", 10)];
  expect(detailToDraft(saved(), codes).code?.code).toBe("VERANO10");
  // el código cambió (ahora 50 %): se conserva el importe guardado como manual
  const changed = detailToDraft(saved(), [code("VERANO10", "percent", 50)]);
  expect(changed.code).toBeNull();
  expect(changed.discount).toBe("6.00");
  expect(draftTotals(changed).discount).toBe(600);
  // código borrado o ausente
  expect(detailToDraft(saved(), []).discount).toBe("6.00");
});

test("la factura se envía y se recupera; un número implica factura", () => {
  expect(emptyDraft()).toMatchObject({ invoiceRequired: false, invoiceNumber: "" });
  const r = draftToInput(base({ invoiceNumber: " F-77 " }));
  expect(r.ok && r.input).toMatchObject({ invoiceRequired: true, invoiceNumber: "F-77" });
  const sin = draftToInput(base());
  expect(sin.ok && sin.input).toMatchObject({ invoiceRequired: false, invoiceNumber: null });
  expect(detailToDraft(saved({ invoiceRequired: true, invoiceNumber: "F-1" }))).toMatchObject({
    invoiceRequired: true,
    invoiceNumber: "F-1",
  });
});

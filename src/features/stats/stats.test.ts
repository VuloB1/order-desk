import { expect, test } from "vitest";
import type { OrderSummary } from "@/lib/api";
import { lastMonths, summarize } from "./stats";

const order = (over: Partial<OrderSummary>): OrderSummary => ({
  id: "x", externalNumber: "#1", status: "paid", orderDate: "2026-11-05", destination: "chile",
  itemCount: 1, subtotalCents: 0, shippingCostCents: 0, discountCents: 0, discountCode: null,
  totalCents: 1000, invoiceRequired: false, invoiceNumber: null, ...over,
});

test("últimos meses cruzando el cambio de año", () => {
  expect(lastMonths("2026-02-10", 4)).toEqual(["2025-11", "2025-12", "2026-01", "2026-02"]);
});

test("las cifras ignoran pedidos cancelados salvo en «por estado»", () => {
  const s = summarize(
    [
      order({ totalCents: 1000 }),
      order({ totalCents: 3000, destination: "foreign", orderDate: "2026-10-20", invoiceRequired: true }),
      order({ totalCents: 9999, status: "cancelled" }),
      order({ totalCents: 500, orderDate: "2020-01-01" }), // fuera de los 6 meses: cuenta en el total, no en el mes
    ],
    "2026-11-15",
  );
  expect(s.orders).toBe(3);
  expect(s.revenueCents).toBe(4500);
  expect(s.averageCents).toBe(1500);
  expect(s.invoicesPending).toBe(1);
  expect(s.byStatus).toMatchObject({ paid: 3, cancelled: 1 });
  expect(s.byDestination.foreign).toEqual({ count: 1, totalCents: 3000 });
  expect(s.months[s.months.length - 1]).toEqual({ month: "2026-11", count: 1, totalCents: 1000 });
  expect(s.months[s.months.length - 2]).toEqual({ month: "2026-10", count: 1, totalCents: 3000 });
});

test("sin pedidos no hay división por cero", () => {
  expect(summarize([], "2026-11-15")).toMatchObject({ orders: 0, averageCents: 0 });
});

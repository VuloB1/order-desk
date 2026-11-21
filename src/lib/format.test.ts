import { expect, test } from "vitest";
import { displayNumber, nextOrderNumber } from "./format";

test("displayNumber antepone # una sola vez y respeta el formato", () => {
  expect(displayNumber("1424")).toBe("#1424");
  expect(displayNumber("#1424")).toBe("#1424");
  expect(displayNumber("#01424")).toBe("#01424");
  expect(displayNumber("A-15")).toBe("#A-15");
});

test("nextOrderNumber suma 1 y conserva el formato", () => {
  expect(nextOrderNumber("#1313")).toBe("#1314");
  expect(nextOrderNumber("1313")).toBe("1314");
  expect(nextOrderNumber("#0099")).toBe("#0100");
  expect(nextOrderNumber("#999")).toBe("#1000");
  expect(nextOrderNumber("A-9")).toBe("A-10");
  expect(nextOrderNumber("#12b")).toBe("#13b");
  expect(nextOrderNumber("#12345678901234567890")).toBe("#12345678901234567891"); // sin perder precisión
  expect(nextOrderNumber("sin número")).toBeNull();
  expect(nextOrderNumber("")).toBeNull();
});

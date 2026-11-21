import { expect, test } from "vitest";
import { centsToInput, parseUsd, todayIso } from "./money";

test("parseUsd acepta coma o punto y rechaza lo inválido", () => {
  expect(parseUsd("12")).toBe(1200);
  expect(parseUsd("12,5")).toBe(1250);
  expect(parseUsd(" 19.99 ")).toBe(1999);
  expect(parseUsd("0.05")).toBe(5);
  for (const bad of ["", "abc", "-1", "1.234", "1,2,3", "1e3"]) expect(parseUsd(bad)).toBeNull();
});

test("centsToInput y todayIso", () => {
  expect(centsToInput(1250)).toBe("12.50");
  expect(todayIso(new Date(2026, 0, 5))).toBe("2026-01-05");
});

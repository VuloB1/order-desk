import { expect, test } from "vitest";
import { backupIsStale } from "./reminder";

const DAY = 86_400_000;

test("el aviso salta sin copia previa o pasados 30 días", () => {
  const now = Date.UTC(2026, 10, 15);
  expect(backupIsStale(null, now)).toBe(true);
  expect(backupIsStale(now - 29 * DAY, now)).toBe(false);
  expect(backupIsStale(now - 31 * DAY, now)).toBe(true);
});

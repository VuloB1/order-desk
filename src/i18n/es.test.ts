import { expect, test } from "vitest";
import { DESTINATIONS, STATUSES } from "@/lib/api";
import { destinationColor, destinationLabel, statusColor, statusLabel } from "./es";

test("cada estado y destino tiene etiqueta con emoji y color propio", () => {
  const emoji = /^\p{Extended_Pictographic}/u;
  for (const s of STATUSES) {
    expect(statusLabel[s]).toMatch(emoji);
    expect(statusColor[s]).toBeTruthy();
  }
  for (const d of DESTINATIONS) {
    expect(destinationLabel[d]).toMatch(emoji);
    expect(destinationColor[d]).toBeTruthy();
  }
  expect(new Set(STATUSES.map((s) => statusColor[s])).size).toBe(STATUSES.length); // colores distintos
});

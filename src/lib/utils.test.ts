import { expect, test } from "vitest";
import { cn } from "./utils";

test("cn fusiona clases de Tailwind", () => {
  expect(cn("p-2", "p-4")).toBe("p-4");
});

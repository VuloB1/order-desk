import { expect, test } from "vitest";
import { emptyPrivate, forDestination, hasPrivate } from "./PrivateFields";

test("en Chile no se guarda el país; en el extranjero sí", () => {
  const d = { ...emptyPrivate, name: "Persona de Prueba", country: "Alemania" };
  expect(forDestination(d, "chile").country).toBeNull();
  expect(forDestination(d, "foreign").country).toBe("Alemania");
  expect(forDestination(d, "chile").name).toBe("Persona de Prueba");
});

test("hasPrivate ignora campos vacíos o con solo espacios", () => {
  expect(hasPrivate(emptyPrivate)).toBe(false);
  expect(hasPrivate({ ...emptyPrivate, name: "   " })).toBe(false);
  expect(hasPrivate({ ...emptyPrivate, city: "Ciudad de Prueba" })).toBe(true);
});

test("el RUT y los datos de empresa cuentan como datos privados", () => {
  expect(hasPrivate({ ...emptyPrivate, rut: "11111111-1" })).toBe(true);
  expect(hasPrivate({ ...emptyPrivate, businessName: "Empresa de Prueba SpA" })).toBe(true);
});

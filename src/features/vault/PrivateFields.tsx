import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { Destination, PrivateData } from "@/lib/api";
import { t } from "@/i18n/es";

export const emptyPrivate: PrivateData = {
  name: null, email: null, phone: null, address: null,
  country: null, region: null, city: null, postalCode: null, deliveryNotes: null,
  rut: null, businessName: null, businessActivity: null,
};

export const hasPrivate = (d: PrivateData) => Object.values(d).some((v) => v && v.trim() !== "");

/** En Chile el país no se guarda: se muestra siempre «🚚 Chile». */
export const forDestination = (d: PrivateData, destination: Destination): PrivateData =>
  destination === "chile" ? { ...d, country: null } : d;

interface Props {
  data: PrivateData;
  onChange: (d: PrivateData) => void;
  destination: Destination;
  /** Con factura se piden también razón social y giro. */
  invoice?: boolean;
  /** Prefijo de los id de los campos (hay más de un formulario posible en pantalla). */
  idPrefix?: string;
}

/** Campos de contacto y envío del pedido; los usan el formulario de pedido nuevo y la ficha del pedido. */
export default function PrivateFields({ data, onChange, destination, invoice = false, idPrefix = "priv" }: Props) {
  const v = t.vault;
  const field = (k: keyof PrivateData, text: string, wide = false) => (
    <div key={k} className={`space-y-1 ${wide ? "col-span-2" : ""}`}>
      <Label htmlFor={`${idPrefix}-${k}`} className="text-xs">{text}</Label>
      <Input
        id={`${idPrefix}-${k}`}
        autoComplete="off"
        value={data[k] ?? ""}
        onChange={(e) => onChange({ ...data, [k]: e.target.value || null })}
      />
    </div>
  );
  return (
    <div className="grid grid-cols-2 gap-2">
      {field("name", v.name)}
      {field("rut", v.rut)}
      {invoice && field("businessName", v.businessName)}
      {invoice && field("businessActivity", v.businessActivity)}
      {field("email", v.email)}
      {field("phone", v.phone)}
      {destination === "foreign" && field("country", v.country)}
      {field("region", v.region)}
      {field("city", v.city)}
      {field("address", v.address, true)}
      {field("postalCode", v.postalCode)}
      {field("deliveryNotes", v.deliveryNotes, true)}
    </div>
  );
}

import { invoke } from "@tauri-apps/api/core";

export const STATUSES = ["pending", "awaiting_chips", "nrf", "classic", "testing", "ready", "shipped", "cancelled"] as const;
export type OrderStatus = (typeof STATUSES)[number];

export const DESTINATIONS = ["chile", "foreign"] as const;
export type Destination = (typeof DESTINATIONS)[number];

export interface Product {
  id: string;
  name: string;
  unitPriceCents: number;
  active: boolean;
}

export interface Item {
  productId: string | null;
  name: string;
  quantity: number;
  unitPriceCents: number;
}

export interface OrderInput {
  externalNumber: string;
  orderDate: string;
  status?: OrderStatus;
  shippingCostCents: number;
  discountCents: number;
  discountCode: string | null;
  destination: Destination;
  invoiceRequired: boolean;
  invoiceNumber: string | null;
  items: Item[];
}

export interface DiscountCode {
  id: string;
  code: string;
  kind: "percent" | "fixed"; // percent: value = % entero; fixed: value = céntimos de USD
  value: number;
  active: boolean;
}

export interface OrderSummary {
  id: string;
  externalNumber: string;
  status: OrderStatus;
  orderDate: string;
  destination: Destination;
  itemCount: number;
  subtotalCents: number;
  shippingCostCents: number;
  discountCents: number;
  discountCode: string | null;
  totalCents: number;
  invoiceRequired: boolean;
  invoiceNumber: string | null;
}

export interface ProductTotal {
  name: string;
  quantity: number;
  totalCents: number;
}

export interface Shipment {
  carrier: string | null;
  trackingNumber: string | null;
  shippedAt: string | null;
  deliveredAt: string | null;
}

export interface OrderDetail extends OrderSummary {
  items: Item[];
  history: { fromStatus: OrderStatus | null; toStatus: OrderStatus; changedAt: string }[];
  shipment: Shipment | null;
}

export interface OrderFilter {
  status?: OrderStatus;
  destination?: Destination;
  search?: string;
}

// Los errores de Rust llegan como texto.
export type VaultStatus = "none" | "locked" | "unlocked";
export interface PrivateData {
  name: string | null;
  email: string | null;
  phone: string | null;
  address: string | null;
  country: string | null;
  region: string | null;
  city: string | null;
  postalCode: string | null;
  deliveryNotes: string | null;
  rut: string | null;
  businessName: string | null;
  businessActivity: string | null;
}

export interface ExportResult {
  path: string;
  count: number;
}

export interface BoardSettings {
  repoUrl: string;
}

export interface PublishResult {
  changed: boolean;
  pagesUrl: string | null;
}

export interface ImportRow {
  row: number;
  number: string;
  kit: string;
  country: string;
  destination: Destination;
  action: "new" | "update" | "same" | "skip";
  note: string | null;
}

export interface ImportResult {
  created: number;
  updated: number;
  skipped: number;
  errors: string[];
}

export interface BackupResult {
  path: string;
  bytes: number;
}

export const errorMessage = (e: unknown) => (typeof e === "string" ? e : e instanceof Error ? e.message : String(e));

export const api = {
  version: () => invoke<string>("app_version"),
  listProducts: () => invoke<Product[]>("list_products"),
  createProduct: (name: string, unitPriceCents: number) => invoke<Product>("create_product", { name, unitPriceCents }),
  updateProduct: (p: Product) =>
    invoke<Product>("update_product", { id: p.id, name: p.name, unitPriceCents: p.unitPriceCents, active: p.active }),
  deleteProduct: (id: string) => invoke<void>("delete_product", { id }),
  reorderProducts: (ids: string[]) => invoke<void>("reorder_products", { ids }),
  listDiscountCodes: () => invoke<DiscountCode[]>("list_discount_codes"),
  createDiscountCode: (code: string, kind: DiscountCode["kind"], value: number) =>
    invoke<DiscountCode>("create_discount_code", { code, kind, value }),
  updateDiscountCode: (c: DiscountCode) =>
    invoke<DiscountCode>("update_discount_code", { id: c.id, code: c.code, kind: c.kind, value: c.value, active: c.active }),
  deleteDiscountCode: (id: string) => invoke<void>("delete_discount_code", { id }),
  listOrders: (filter: OrderFilter) => invoke<OrderSummary[]>("list_orders", { filter }),
  topProducts: (limit: number) => invoke<ProductTotal[]>("top_products", { limit }),
  getOrder: (id: string) => invoke<OrderDetail>("get_order", { id }),
  createOrder: (input: OrderInput) => invoke<OrderDetail>("create_order", { input }),
  updateOrder: (id: string, input: OrderInput) => invoke<OrderDetail>("update_order", { id, input }),
  changeStatus: (id: string, status: OrderStatus) => invoke<OrderDetail>("change_order_status", { id, status }),
  setShipment: (id: string, shipment: Shipment) => invoke<OrderDetail>("set_shipment", { id, shipment }),
  deleteOrder: (id: string) => invoke<void>("delete_order", { id }),
  lastOrderNumber: () => invoke<string | null>("last_order_number"),
  /** Abre "Guardar como" y escribe el fichero; null si se cancela. */
  getBoardSettings: () => invoke<BoardSettings>("get_board_settings"),
  setBoardSettings: (repoUrl: string) => invoke<BoardSettings>("set_board_settings", { repoUrl }),
  publishBoard: () => invoke<PublishResult>("publish_board"),
  importPreview: () => invoke<ImportRow[] | null>("import_preview"),
  importApply: (rows: ImportRow[], status: OrderStatus, date: string) =>
    invoke<ImportResult>("import_apply", { rows, status, date }),
  exportOrders: (format: "json" | "csv", filter: OrderFilter, fileName: string) =>
    invoke<ExportResult | null>("export_orders", { format, filter, fileName }),
  /** Requiere la sección privada desbloqueada. Abre "Guardar como"; null si se cancela. */
  backupCreate: (fileName: string) => invoke<BackupResult | null>("backup_create", { fileName }),
  /** Abre "Elegir copia" y reemplaza los datos; null si se cancela. */
  backupRestore: (kind: "password" | "recovery", secret: string) =>
    invoke<{ orders: number } | null>("backup_restore", { kind, secret }),
  /** Consulta la última versión publicada; null si ya tienes la última. */
  checkUpdate: () => invoke<{ version: string; notes: string | null } | null>("check_update"),
  /** Descarga e instala; la app se cierra y se reabre sola. */
  installUpdate: () => invoke<void>("install_update"),
  vaultStatus: () => invoke<VaultStatus>("vault_status"),
  /** Devuelve la clave de recuperación: se muestra una sola vez. */
  vaultCreate: (password: string) => invoke<string>("vault_create", { password }),
  vaultUnlock: (password: string) => invoke<void>("vault_unlock", { password }),
  vaultResetPassword: (recoveryKey: string, newPassword: string) =>
    invoke<void>("vault_reset_password", { recoveryKey, newPassword }),
  vaultChangePassword: (oldPassword: string, newPassword: string) =>
    invoke<void>("vault_change_password", { oldPassword, newPassword }),
  vaultLock: () => invoke<void>("vault_lock"),
  getPrivateData: (orderId: string) => invoke<PrivateData | null>("get_private_data", { orderId }),
  setPrivateData: (orderId: string, data: PrivateData) => invoke<void>("set_private_data", { orderId, data }),
};

/** Marca de Order Desk: dos círculos («Order» y «Desk») que se cruzan. */
export default function Logo({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 32 32" aria-hidden className={className}>
      <circle cx="12" cy="16" r="9.5" fill="#ED93B1" />
      <circle cx="21" cy="16" r="9.5" fill="#7F77DD" />
      <path d="M16.5 7.633A9.5 9.5 0 0 1 16.5 24.367 9.5 9.5 0 0 1 16.5 7.633Z" fill="#4B1528" />
    </svg>
  );
}

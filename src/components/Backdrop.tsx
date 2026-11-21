// Estrellas de cuatro puntas (x %, y %, tamaño px, retraso s, duración s). Van sobre todo por los márgenes
// y las esquinas; quedan detrás del contenido, así que nunca estorban.
const STARS: [number, number, number, number, number][] = [
  [5, 13, 18, 0, 5],
  [93, 9, 12, 1.2, 6],
  [89, 33, 22, 2.4, 7],
  [3, 45, 10, 0.6, 5.5],
  [95, 61, 14, 3, 6.5],
  [7, 79, 20, 1.8, 7],
  [91, 87, 12, 0.3, 5],
  [49, 94, 10, 2.1, 6],
  [21, 7, 9, 3.3, 5.8],
  [71, 5, 11, 0.9, 6.2],
  [2, 27, 8, 2.7, 5.2],
  [58, 52, 8, 1.5, 6.8],
];

/** Fondo fijo de toda la aplicación: cuadrícula suave, resplandores y estrellas. Solo decorativo. */
export default function Backdrop() {
  return (
    <div aria-hidden className="app-backdrop">
      {STARS.map(([x, y, size, delay, dur], i) => (
        <svg
          key={i}
          className="star"
          viewBox="0 0 24 24"
          style={{ left: `${x}%`, top: `${y}%`, width: size, height: size, animationDelay: `${delay}s`, animationDuration: `${dur}s` }}
        >
          <path d="M12 0C12.9 7.1 16.9 11.1 24 12C16.9 12.9 12.9 16.9 12 24C11.1 16.9 7.1 12.9 0 12C7.1 11.1 11.1 7.1 12 0Z" />
        </svg>
      ))}
    </div>
  );
}

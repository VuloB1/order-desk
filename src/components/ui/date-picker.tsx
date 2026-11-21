import { useState } from "react"
import { CalendarDays, ChevronLeft, ChevronRight } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { todayIso } from "@/lib/money"
import { cn } from "cn"

const WEEKDAYS = ["L", "M", "X", "J", "V", "S", "D"] // la semana empieza en lunes
const pad = (n: number) => String(n).padStart(2, "0")
const iso = (y: number, m: number, d: number) => `${y}-${pad(m + 1)}-${pad(d)}`

/** AAAA-MM-DD → DD/MM/AAAA (solo para mostrar). */
export const formatDate = (v: string) => (v ? v.split("-").reverse().join("/") : "")

const parts = (v: string): [number, number, number] | null => {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(v)
  return m ? [Number(m[1]), Number(m[2]) - 1, Number(m[3])] : null
}

interface Props {
  value: string // AAAA-MM-DD o ""
  onChange: (v: string) => void
  /** Permite dejar la fecha vacía (botón "Quitar"). */
  clearable?: boolean
  placeholder?: string
  id?: string
  className?: string
  "aria-label"?: string
}

/** Selector de fecha con el diseño de la app (sustituye al <input type="date"> nativo). */
export function DatePicker({ value, onChange, clearable, placeholder = "Elegir fecha", id, className, ...rest }: Props) {
  const [open, setOpen] = useState(false)
  const today = todayIso()
  const start = parts(value) ?? parts(today)!
  const [view, setView] = useState({ y: start[0], m: start[1] })

  const shift = (delta: number) =>
    setView(({ y, m }) => {
      const t = y * 12 + m + delta
      return { y: Math.floor(t / 12), m: t % 12 }
    })
  const pick = (v: string) => {
    onChange(v)
    setOpen(false)
  }

  const first = (new Date(view.y, view.m, 1).getDay() + 6) % 7 // lunes = 0
  const days = new Date(view.y, view.m + 1, 0).getDate()
  const cells = [...Array<null>(first).fill(null), ...Array.from({ length: days }, (_, i) => i + 1)]
  const label = new Intl.DateTimeFormat("es-CL", { month: "long", year: "numeric" }).format(new Date(view.y, view.m, 1))

  return (
    <Popover
      open={open}
      onOpenChange={(o) => {
        if (o) {
          const s = parts(value) ?? parts(today)!
          setView({ y: s[0], m: s[1] })
        }
        setOpen(o)
      }}
    >
      <PopoverTrigger
        render={
          <Button id={id} type="button" variant="outline" aria-label={rest["aria-label"]} className={cn("justify-start font-normal", className)} />
        }
      >
        <CalendarDays data-icon="inline-start" />
        {value ? formatDate(value) : <span className="text-muted-foreground">{placeholder}</span>}
      </PopoverTrigger>
      <PopoverContent align="start" className="w-64 space-y-2 p-3">
        <div className="flex items-center justify-between">
          <Button type="button" variant="ghost" size="icon-sm" aria-label="Mes anterior" onClick={() => shift(-1)}><ChevronLeft /></Button>
          <span className="text-sm font-medium first-letter:uppercase">{label}</span>
          <Button type="button" variant="ghost" size="icon-sm" aria-label="Mes siguiente" onClick={() => shift(1)}><ChevronRight /></Button>
        </div>
        <div className="grid grid-cols-7 gap-1 text-center text-xs text-muted-foreground">
          {WEEKDAYS.map((w) => <span key={w}>{w}</span>)}
        </div>
        <div className="grid grid-cols-7 gap-1">
          {cells.map((d, i) =>
            d === null ? (
              <span key={`b${i}`} />
            ) : (
              <Button
                key={d}
                type="button"
                size="icon-sm"
                variant={iso(view.y, view.m, d) === value ? "default" : "ghost"}
                className={cn("w-full font-normal", iso(view.y, view.m, d) === today && iso(view.y, view.m, d) !== value && "ring-1 ring-primary/60")}
                onClick={() => pick(iso(view.y, view.m, d))}
              >
                {d}
              </Button>
            ),
          )}
        </div>
        <div className="flex justify-between pt-1">
          <Button type="button" variant="link" size="sm" className="px-0" onClick={() => pick(today)}>Hoy</Button>
          {clearable && value && <Button type="button" variant="link" size="sm" className="px-0" onClick={() => pick("")}>Quitar</Button>}
        </div>
      </PopoverContent>
    </Popover>
  )
}

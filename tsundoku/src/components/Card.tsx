import type { ReactNode } from "react"

/** A raised block inside a view, such as a tool in Settings. Its padding and layout come from `className`. */
export function Card({ id, className, children }: { id?: string; className?: string; children: ReactNode }) {
  return <div id={id} className={["rounded-lg border border-line bg-card", className].filter(Boolean).join(" ")}>{children}</div>
}

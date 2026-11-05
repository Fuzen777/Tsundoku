import type { ReactNode } from "react"

/** A titled group within a view, such as Tools in Settings. */
export function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-2">
      <h2 className="text-sm font-semibold">{title}</h2>
      {children}
    </section>
  )
}

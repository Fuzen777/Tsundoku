import type { ReactNode } from "react"

/** A view's frame: its title, then content that scrolls on its own. */
export function Page({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <section className="flex h-full flex-col">
      <h1 className="px-6 pt-5 pb-3 text-xl font-semibold">{title}</h1>
      <div className="min-h-0 flex-1 overflow-auto px-6 pb-6">{children}</div>
    </section>
  )
}

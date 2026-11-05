import type { ReactNode } from "react"

interface PageProps {
  title: string
  /** Buttons for the whole view, on the title's right. */
  actions?: ReactNode
  children?: ReactNode
}

/** A view's frame: its title, then content that scrolls on its own. */
export function Page({ title, actions, children }: PageProps) {
  return (
    <section className="flex h-full flex-col">
      <header className="flex items-center justify-between gap-4 px-6 pt-5 pb-3">
        <h1 className="text-xl font-semibold">{title}</h1>
        {actions && <div className="flex items-center gap-2">{actions}</div>}
      </header>
      <div className="min-h-0 flex-1 overflow-auto px-6 pb-6">{children}</div>
    </section>
  )
}

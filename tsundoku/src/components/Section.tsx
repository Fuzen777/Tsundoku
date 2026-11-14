import type { ReactNode } from "react"

interface SectionProps {
  title: string
  /** On the title's right, such as a `HelpMenu`. */
  actions?: ReactNode
  children: ReactNode
}

/** A titled group within a view, such as Tools in Settings. */
export function Section({ title, actions, children }: SectionProps) {
  const heading = <h2 className="text-sm font-semibold">{title}</h2>
  return (
    <section className="flex flex-col gap-2">
      {actions ? (
        <div className="flex items-center justify-between gap-2">
          {heading}
          {/* The actions' 32 px buttons overlap the space around the title, so the section keeps its height. */}
          <div className="-my-2 flex items-center gap-1">{actions}</div>
        </div>
      ) : (
        heading
      )}
      {children}
    </section>
  )
}

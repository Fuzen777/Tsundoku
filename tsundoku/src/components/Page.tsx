import type { ReactNode, Ref, UIEventHandler } from "react"

interface PageProps {
  title: string
  /** Buttons for the whole view, on the title's right. */
  actions?: ReactNode
  /** How the view shows its list (`PickerViewControls`), at the header's right end, after the actions, so it stays
   * where it is whatever the actions are, and in sight however far the list is scrolled. */
  viewControls?: ReactNode
  /** The element that scrolls, for a view that draws only the rows on screen. */
  scrollRef?: Ref<HTMLDivElement>
  /** Called as the content scrolls, such as for Settings to tell the sidebar which section shows. */
  onScroll?: UIEventHandler<HTMLDivElement>
  children?: ReactNode
}

/** A view's frame: its title, then content that scrolls on its own. */
export function Page({ title, actions, viewControls, scrollRef, onScroll, children }: PageProps) {
  return (
    <section className="flex h-full flex-col">
      <header className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2 px-6 pt-5 pb-3">
        <h1 className="text-xl font-semibold">{title}</h1>
        {(actions || viewControls) && (
          <div className="flex flex-wrap items-center justify-end gap-2">
            {actions}
            {/* Set apart from the actions, and wrapping under them as one when the window is narrow. */}
            {viewControls && <div className={actions ? "ml-2" : undefined}>{viewControls}</div>}
          </div>
        )}
      </header>
      {/* The scrollbar's room is kept while there's nothing to scroll, so nothing moves sideways when it comes: a list
          growing longer, or the details panel taking room from it. */}
      <div ref={scrollRef} onScroll={onScroll} className="min-h-0 flex-1 overflow-auto px-6 pb-6 [scrollbar-gutter:stable]">
        {children}
      </div>
    </section>
  )
}

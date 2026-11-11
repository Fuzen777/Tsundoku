import { CircleHelp } from "lucide-react"
import { Popover } from "./Popover"

export interface HelpTopic {
  /** The field or idea explained, as the view names it. */
  title: string
  paragraphs: readonly string[]
}

interface HelpMenuProps {
  /** What it explains, for screen readers and the button's tooltip. */
  label: string
  topics: readonly HelpTopic[]
}

/** A (?) icon button opening what a view's fields mean, each as a heading and a few paragraphs, so the view's own cards
 * can leave the explanations out. It scrolls when it's taller than the window, and closes like a `Menu`. */
export function HelpMenu({ label, topics }: HelpMenuProps) {
  return (
    <Popover
      label={label}
      icon={CircleHelp}
      role="dialog"
      fitsWindow
      panelClassName="flex w-md max-w-[calc(100vw-3rem)] flex-col gap-4 overflow-y-auto p-4"
    >
      {() =>
        topics.map((topic) => (
          <section key={topic.title} className="flex flex-col gap-1.5">
            <h2 className="font-semibold">{topic.title}</h2>
            {topic.paragraphs.map((paragraph) => (
              <p key={paragraph} className="text-fg-secondary">
                {paragraph}
              </p>
            ))}
          </section>
        ))
      }
    </Popover>
  )
}

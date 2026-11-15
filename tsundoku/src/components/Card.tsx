import type { HTMLAttributes } from "react"

/** A raised block inside a view, such as a tool in Settings. Its padding and layout come from `className`; what else a
 * div takes (an id, a double-click) passes through. */
export function Card({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div {...props} className={["rounded-lg border border-line bg-card", className].filter(Boolean).join(" ")} />
}

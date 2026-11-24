import { useContext, type HTMLAttributes } from "react"
import { FlatCardContext } from "../tileRows"

/** A raised block inside a view, such as a tool in Settings. Its padding and layout come from `className`; what else a
 * div takes (an id, a double-click) passes through. In the details panel, the outermost one is the panel itself
 * (`FlatCardContext`). */
export function Card({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  const flat = useContext(FlatCardContext)
  const card = (
    <div {...props} className={[!flat && "rounded-lg border border-line bg-card", className].filter(Boolean).join(" ")} />
  )
  return flat ? <FlatCardContext value={false}>{card}</FlatCardContext> : card
}

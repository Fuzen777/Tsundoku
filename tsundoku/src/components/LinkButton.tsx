import { ChevronDown, ChevronRight, type LucideIcon } from "lucide-react"

const LINK_CLASSES =
  "flex w-fit items-center gap-1 rounded text-xs font-medium text-accent hover:text-accent-hover focus-visible:outline-2 focus-visible:outline-accent"

/** A small accent link with an icon before its label, such as a release's page. */
export function LinkButton({ icon: Icon, label, onClick }: { icon: LucideIcon; label: string; onClick: () => void }) {
  return (
    <button type="button" onClick={onClick} className={LINK_CLASSES}>
      <Icon size={14} />
      {label}
    </button>
  )
}

/** The link that shows or hides what's under it: a history row's files or downloads, the update banner's notes. */
export function ExpandToggle({ expanded, onToggle, label }: { expanded: boolean; onToggle: () => void; label: string }) {
  return (
    <button type="button" aria-expanded={expanded} onClick={onToggle} className={LINK_CLASSES}>
      {expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
      {label}
    </button>
  )
}

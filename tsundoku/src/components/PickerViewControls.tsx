import type { LucideIcon } from "lucide-react"
import { LayoutDashboard, LayoutGrid, LayoutList, Minus, Plus } from "lucide-react"
import type { PickerLayout } from "../bindings/PickerLayout"
import type { PickerView } from "../bindings/PickerView"
import { resized } from "../pickerLayout"
import { strings } from "../strings"
import { SegmentedControl } from "./SegmentedControl"
import { Tooltip } from "./Tooltip"

const text = strings.pickerView

const VIEWS: readonly { value: PickerView; label: string; icon: LucideIcon }[] = [
  { value: "list", label: text.views.list, icon: LayoutList },
  { value: "masonry", label: text.views.masonry, icon: LayoutDashboard },
  { value: "gallery", label: text.views.gallery, icon: LayoutGrid },
]

interface PickerViewControlsProps {
  layout: PickerLayout
  /** Changes the latest layout, which a quick second click builds on. */
  onChange: (update: (layout: PickerLayout) => PickerLayout) => void
}

/**
 * A picker's view as icons (List, Masonry, Gallery) and, in the two views of pictures, − and + before them, which make
 * the pictures smaller or larger. They sit before the views, so the views stay where they are when they come and go.
 */
export function PickerViewControls({ layout, onChange }: PickerViewControlsProps) {
  const step = (by: 1 | -1) => resized(layout, by) && (() => onChange((latest) => resized(latest, by) ?? latest))
  return (
    <div className="flex shrink-0 items-center gap-2">
      {layout.view !== "list" && (
        <div role="group" aria-label={text.size} className="inline-flex rounded-md border border-line bg-input p-0.5">
          <StepButton label={text.smaller} icon={Minus} onClick={step(-1)} />
          <StepButton label={text.larger} icon={Plus} onClick={step(1)} />
        </div>
      )}
      <SegmentedControl
        label={text.view}
        options={VIEWS}
        value={layout.view}
        onChange={(view) => onChange((latest) => ({ ...latest, view }))}
      />
    </div>
  )
}

/** One of − and +, off at the end of the sizes (`onClick` null). */
function StepButton({ label, icon: Icon, onClick }: { label: string; icon: LucideIcon; onClick: (() => void) | null }) {
  return (
    <Tooltip label={label}>
      <button
        type="button"
        aria-label={label}
        disabled={!onClick}
        onClick={onClick ?? undefined}
        className="inline-flex h-6 items-center rounded px-2 text-fg-secondary transition-colors hover:text-fg focus-visible:outline-2 focus-visible:outline-accent disabled:opacity-40"
      >
        <Icon size={14} />
      </button>
    </Tooltip>
  )
}

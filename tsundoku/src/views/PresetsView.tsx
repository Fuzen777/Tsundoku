import { useState } from "react"
import { useQueryClient } from "@tanstack/react-query"
import { confirmWithUser, queryKeys } from "../api"
import type { Preset } from "../bindings/Preset"
import { Page } from "../components/Page"
import { classes } from "../components/styles"
import { usePresets } from "../queries"
import { strings } from "../strings"
import { PresetEditor } from "./presets/PresetEditor"
import { presetSummary } from "./presets/presetLabels"

/** The preset list, and the editor for the one selected. Edits stay a draft until Save. */
export function PresetsView() {
  const { data: overview } = usePresets()
  const queryClient = useQueryClient()
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [draft, setDraft] = useState<Preset | null>(null)

  if (!overview) return <Page title={strings.nav.presets} />
  const selected = overview.presets.find((preset) => preset.id === selectedId) ?? overview.presets[0]
  const isDirty = draft !== null && JSON.stringify(draft) !== JSON.stringify(selected)

  async function select(id: string) {
    if (id === selected.id) return
    if (
      isDirty &&
      !(await confirmWithUser({
        title: strings.presets.discardTitle,
        message: strings.presets.discardMessage(selected.name),
        confirmLabel: strings.presets.discardConfirm,
        cancelLabel: strings.dialogs.cancel,
      }))
    ) {
      return
    }
    setDraft(null)
    setSelectedId(id)
  }

  return (
    <Page title={strings.nav.presets}>
      <div className="grid grid-cols-[14rem_minmax(0,1fr)] items-start gap-4">
        {/* Stays in view while the editor beside it scrolls. */}
        <nav className="sticky top-0 flex flex-col gap-1">
          {overview.presets.map((preset) => {
            const isSelected = preset.id === selected.id
            return (
              <button
                key={preset.id}
                type="button"
                aria-current={isSelected ? "true" : undefined}
                onClick={() => void select(preset.id)}
                className={classes(
                  "flex flex-col rounded-md px-3 py-2 text-left transition-colors focus-visible:outline-2 focus-visible:outline-accent",
                  isSelected ? "bg-input" : "hover:bg-input",
                )}
              >
                <span className="flex items-center gap-2 font-medium">
                  <span className="truncate">{isSelected && draft ? draft.name || preset.name : preset.name}</span>
                  {preset.id === overview.defaultId && (
                    <span className="shrink-0 rounded-full border border-line px-1.5 text-xs font-normal text-fg-secondary">
                      {strings.presets.defaultBadge}
                    </span>
                  )}
                </span>
                <span className="truncate text-xs text-fg-secondary">{presetSummary(preset)}</span>
              </button>
            )
          })}
        </nav>
        <PresetEditor
          key={selected.id}
          saved={selected}
          draft={draft ?? selected}
          isDirty={isDirty}
          overview={overview}
          onChange={setDraft}
          onSaved={() => setDraft(null)}
          onSelect={(id) => {
            setDraft(null)
            setSelectedId(id)
            void queryClient.invalidateQueries({ queryKey: queryKeys.presets })
          }}
        />
      </div>
    </Page>
  )
}

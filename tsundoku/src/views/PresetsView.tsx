import { useState } from "react"
import { useQueryClient } from "@tanstack/react-query"
import { Plus, RotateCcw } from "lucide-react"
import { addPreset, confirmWithUser, queryKeys, resetPreset } from "../api"
import { Button } from "../components/Button"
import { Menu } from "../components/Menu"
import { Page } from "../components/Page"
import { StatusMessage } from "../components/StatusMessage"
import { describePresetError, type ErrorDescription } from "../errors"
import { classes } from "../components/styles"
import { usePresets, useStorePresets } from "../queries"
import { strings } from "../strings"
import { PresetEditor } from "./presets/PresetEditor"
import { presetSummary } from "./presets/presetLabels"
import { usePresetDraft } from "./presets/usePresetDraft"

/** The preset list, and the editor for the one selected. Edits save on their own once they pause. */
export function PresetsView() {
  const { data: overview } = usePresets()
  const queryClient = useQueryClient()
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const { draft, saveState, saveError, change, save, discard } = usePresetDraft()
  const storePresets = useStorePresets()
  const [listError, setListError] = useState<ErrorDescription | null>(null)

  if (!overview) return <Page title={strings.nav.presets} />
  const selected = overview.presets.find((preset) => preset.id === selectedId) ?? overview.presets[0]

  /** Saves the open preset's edits before another opens, or asks to drop them when they can't be saved. */
  async function leaveOpenPreset() {
    if (await save()) return true
    return confirmWithUser({
      title: strings.presets.discardTitle,
      message: strings.presets.discardMessage(selected.name),
      confirmLabel: strings.presets.discardConfirm,
      cancelLabel: strings.dialogs.cancel,
    })
  }

  async function select(id: string) {
    if (id === selected.id) return
    if (!(await leaveOpenPreset())) return
    discard()
    setSelectedId(id)
  }

  /** Adds a preset, or puts back a deleted built-in, then opens it. */
  async function addThenSelect(add: () => Promise<string>) {
    setListError(null)
    if (!(await leaveOpenPreset())) return
    try {
      const id = await add()
      void queryClient.invalidateQueries({ queryKey: queryKeys.presets })
      discard()
      setSelectedId(id)
    } catch (error) {
      setListError(describePresetError(error))
    }
  }
  const addNew = () => addThenSelect(async () => (await addPreset(strings.presets.newName)).id)
  const putBack = (id: string) =>
    addThenSelect(async () => {
      storePresets(await resetPreset(id))
      return id
    })

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
          <div className="mt-2 flex items-center gap-1">
            <Button size="sm" onClick={() => void addNew()}>
              <Plus className="size-4" aria-hidden />
              {strings.presets.add}
            </Button>
            {overview.removedBuiltIns.length > 0 && (
              <Menu
                label={strings.presets.restoreBuiltIn}
                icon={RotateCcw}
                items={overview.removedBuiltIns.map((preset) => ({
                  label: strings.presets.restoreItem(preset.name),
                  onSelect: () => void putBack(preset.id),
                }))}
              />
            )}
          </div>
          {listError && <StatusMessage tone="danger" {...listError} />}
        </nav>
        <PresetEditor
          key={selected.id}
          saved={selected}
          draft={draft ?? selected}
          overview={overview}
          saveState={saveState}
          saveError={saveError}
          onChange={change}
          onSaveNow={save}
          onDiscard={discard}
          onSelect={(id) => {
            discard()
            setSelectedId(id)
            void queryClient.invalidateQueries({ queryKey: queryKeys.presets })
          }}
        />
      </div>
    </Page>
  )
}

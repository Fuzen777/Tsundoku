import { useCallback, useEffect, useRef, useState } from "react"
import { savePreset } from "../../api"
import type { Preset } from "../../bindings/Preset"
import { describePresetError, type ErrorDescription } from "../../errors"
import { useStorePresets } from "../../queries"

/** How long after the last edit it's saved. */
const AUTO_SAVE_DELAY = 600

export type PresetSaveState = "idle" | "saving" | "saved"

/**
 * The preset being edited, saved on its own once edits pause. `save` saves what's waiting at once (before switching
 * presets, say) and resolves to whether it worked; `discard` drops it. Saves run one after another, so an older
 * answer never replaces a newer one, and what's waiting is saved when the editor goes away.
 */
export function usePresetDraft() {
  const storePresets = useStorePresets()
  const [draft, setDraft] = useState<Preset | null>(null)
  const [saveState, setSaveState] = useState<PresetSaveState>("idle")
  const [saveError, setSaveError] = useState<ErrorDescription | null>(null)
  /** The latest edit not saved yet. */
  const waiting = useRef<Preset | null>(null)
  const timer = useRef<number | undefined>(undefined)
  const lastSave = useRef<Promise<boolean>>(Promise.resolve(true))

  const saveWaiting = useCallback(async (): Promise<boolean> => {
    const preset = waiting.current
    if (!preset) return true
    setSaveState("saving")
    try {
      storePresets(await savePreset(preset))
      // Edits made meanwhile stay a draft, for the next save.
      if (waiting.current === preset) {
        waiting.current = null
        setDraft(null)
      }
      setSaveError(null)
      setSaveState("saved")
      return true
    } catch (error) {
      setSaveError(describePresetError(error))
      setSaveState("idle")
      return false
    }
  }, [storePresets])

  const save = useCallback((): Promise<boolean> => {
    window.clearTimeout(timer.current)
    lastSave.current = lastSave.current.then(saveWaiting)
    return lastSave.current
  }, [saveWaiting])

  const change = useCallback(
    (preset: Preset) => {
      waiting.current = preset
      setDraft(preset)
      setSaveState("idle")
      window.clearTimeout(timer.current)
      timer.current = window.setTimeout(() => void save(), AUTO_SAVE_DELAY)
    },
    [save],
  )

  const discard = useCallback(() => {
    window.clearTimeout(timer.current)
    waiting.current = null
    setDraft(null)
    setSaveError(null)
    setSaveState("idle")
  }, [])

  // Leaving the view saves what's waiting rather than losing it.
  useEffect(() => () => void save(), [save])

  return { draft, saveState, saveError, change, save, discard }
}

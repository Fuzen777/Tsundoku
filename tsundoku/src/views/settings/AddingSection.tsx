import type { ClipboardShortcut } from "../../bindings/ClipboardShortcut"
import { Card } from "../../components/Card"
import { Checkbox, Field, SelectInput } from "../../components/Field"
import { Section } from "../../components/Section"
import { StatusMessage } from "../../components/StatusMessage"
import { useSaveAddingOptions } from "../../queries"
import { strings } from "../../strings"

const text = strings.settings

/** The shortcuts to pick from, in the order offered. */
const SHORTCUTS = Object.keys(text.shortcutKeys) as ClipboardShortcut[]

/**
 * Settings → Adding Links: the clipboard watcher, the clipboard shortcut, Download Right Away for the links either
 * adds, and notifications.
 */
export function AddingSection() {
  const { options, save, error } = useSaveAddingOptions()
  const watching = options?.watchClipboard ?? false
  const shortcut = options?.clipboardShortcut ?? null

  return (
    <Section title={text.adding}>
      <Card className="flex flex-col gap-4 p-4">
        <div className="flex flex-col gap-1">
          <Checkbox
            label={text.watchClipboard}
            checked={watching}
            disabled={!options}
            onChange={(checked) => void save({ watchClipboard: checked })}
          />
          <p className="pl-6 text-xs text-fg-muted">{text.watchClipboardHint}</p>
        </div>
        <div className="max-w-xs">
          <Field label={text.clipboardShortcut} hint={text.clipboardShortcutHint} htmlFor="settings-clipboard-shortcut">
            <SelectInput
              id="settings-clipboard-shortcut"
              value={shortcut ?? ""}
              disabled={!options}
              onChange={(event) =>
                void save({ clipboardShortcut: (event.target.value || null) as ClipboardShortcut | null })
              }
            >
              <option value="">{text.noShortcut}</option>
              {SHORTCUTS.map((choice) => (
                <option key={choice} value={choice}>
                  {text.shortcutKeys[choice]}
                </option>
              ))}
            </SelectInput>
          </Field>
        </div>
        <div className="flex flex-col gap-1">
          <Checkbox
            label={text.downloadRightAway}
            checked={options?.downloadRightAway ?? false}
            disabled={!options || (!watching && shortcut === null)}
            onChange={(checked) => void save({ downloadRightAway: checked })}
          />
          <p className="pl-6 text-xs text-fg-muted">{text.downloadRightAwayHint}</p>
        </div>
        <div className="flex flex-col gap-1">
          <Checkbox
            label={text.notify}
            checked={options?.notify ?? true}
            disabled={!options}
            onChange={(checked) => void save({ notify: checked })}
          />
          <p className="pl-6 text-xs text-fg-muted">{text.notifyHint}</p>
        </div>
        {error && <StatusMessage tone="danger" {...error} />}
      </Card>
    </Section>
  )
}

import { useState } from "react"
import { copyText } from "../../api"
import type { ClipboardShortcut } from "../../bindings/ClipboardShortcut"
import { Button } from "../../components/Button"
import { Card } from "../../components/Card"
import { Checkbox, Field, SelectInput } from "../../components/Field"
import { Section } from "../../components/Section"
import { StatusMessage } from "../../components/StatusMessage"
import { useSaveAddingOptions } from "../../queries"
import { SEND_PAGE_BOOKMARK } from "../../sendPage"
import { strings } from "../../strings"

const text = strings.settings

/** The shortcuts to pick from, in the order offered. */
const SHORTCUTS = Object.keys(text.shortcutKeys) as ClipboardShortcut[]

/**
 * Settings → Adding Links: the clipboard watcher, the clipboard shortcut, the browser bookmark, Download Right Away
 * for the links they add, and notifications.
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
        <BrowserBookmark />
        <div className="flex flex-col gap-1">
          <Checkbox
            label={text.downloadRightAway}
            checked={options?.downloadRightAway ?? false}
            disabled={!options}
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
        <div className="flex flex-col gap-1">
          <Checkbox
            label={text.notifyFailures}
            checked={options?.notifyFailures ?? true}
            disabled={!options}
            onChange={(checked) => void save({ notifyFailures: checked })}
          />
          <p className="pl-6 text-xs text-fg-muted">{text.notifyFailuresHint}</p>
        </div>
        <div className="flex flex-col gap-1">
          <Checkbox
            label={text.notifyFinished}
            checked={options?.notifyFinished ?? true}
            disabled={!options}
            onChange={(checked) => void save({ notifyFinished: checked })}
          />
          <p className="pl-6 text-xs text-fg-muted">{text.notifyFinishedHint}</p>
        </div>
        {error && <StatusMessage tone="danger" {...error} />}
      </Card>
    </Section>
  )
}

/** Send from Your Browser: what the bookmark does and how to add it, with Copy Bookmark Address. */
function BrowserBookmark() {
  const [copied, setCopied] = useState<boolean | null>(null)

  async function copy() {
    try {
      await copyText(SEND_PAGE_BOOKMARK)
      setCopied(true)
    } catch {
      setCopied(false)
    }
  }

  return (
    <div className="flex flex-col gap-1.5">
      <p className="font-medium">{text.browserBookmark}</p>
      <p className="text-xs text-fg-muted">{text.browserBookmarkHint}</p>
      <div className="flex items-center gap-3">
        <Button size="sm" onClick={() => void copy()}>
          {text.copyBookmark}
        </Button>
        {copied === true && <StatusMessage tone="success" message={text.bookmarkCopied} />}
        {copied === false && <StatusMessage tone="danger" message={strings.errors.unexpected} />}
      </div>
    </div>
  )
}

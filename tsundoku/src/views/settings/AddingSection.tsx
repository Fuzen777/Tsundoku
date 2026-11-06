import { useState } from "react"
import { copyText } from "../../api"
import type { ClipboardShortcut } from "../../bindings/ClipboardShortcut"
import { Button } from "../../components/Button"
import { Card } from "../../components/Card"
import { Checkbox, Field, SelectInput } from "../../components/Field"
import { Section } from "../../components/Section"
import { StatusMessage } from "../../components/StatusMessage"
import { SETTINGS_PLACES } from "../../navigation"
import { useSaveAddingOptions } from "../../queries"
import { SEND_BUTTONS_USERSCRIPT, SEND_PAGE_BOOKMARK } from "../../sendPage"
import { strings } from "../../strings"

const text = strings.settings

/** The shortcuts to pick from, in the order offered. */
const SHORTCUTS = Object.keys(text.shortcutKeys) as ClipboardShortcut[]

/**
 * Settings → Adding Links: the clipboard watcher, the clipboard shortcut, the browser bookmark and the send buttons'
 * userscript, Download Right Away for the links they add, and notifications.
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
          <Field label={text.clipboardShortcut} hint={text.clipboardShortcutHint} htmlFor={SETTINGS_PLACES.clipboardShortcut}>
            <SelectInput
              id={SETTINGS_PLACES.clipboardShortcut}
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
        <CopyBlock
          id={SETTINGS_PLACES.sendPage}
          title={text.browserBookmark}
          hint={text.browserBookmarkHint}
          copyLabel={text.copyBookmark}
          copiedMessage={text.bookmarkCopied}
          value={SEND_PAGE_BOOKMARK}
        />
        <CopyBlock
          id={SETTINGS_PLACES.sendButtons}
          title={text.sendButtons}
          hint={text.sendButtonsHint}
          howTo={text.userscriptHowTo}
          copyLabel={text.copyUserscript}
          copiedMessage={text.userscriptCopied}
          value={SEND_BUTTONS_USERSCRIPT}
        />
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

interface CopyBlockProps {
  /** Where Settings scrolls to when another view opens it there. */
  id: string
  title: string
  hint: string
  /** How to use what's copied, under the button. */
  howTo?: string
  copyLabel: string
  copiedMessage: string
  value: string
}

/** A way to add links from a browser: what it does, a button that copies what it takes, and how to use it. */
function CopyBlock({ id, title, hint, howTo, copyLabel, copiedMessage, value }: CopyBlockProps) {
  const [copied, setCopied] = useState<boolean | null>(null)

  async function copy() {
    try {
      await copyText(value)
      setCopied(true)
    } catch {
      setCopied(false)
    }
  }

  return (
    <div id={id} className="flex scroll-mt-4 flex-col gap-1.5">
      <p className="font-medium">{title}</p>
      <p className="text-xs text-fg-muted">{hint}</p>
      <div className="flex items-center gap-3">
        <Button size="sm" onClick={() => void copy()}>
          {copyLabel}
        </Button>
        {copied === true && <StatusMessage tone="success" message={copiedMessage} />}
        {copied === false && <StatusMessage tone="danger" message={strings.errors.unexpected} />}
      </div>
      {howTo && <p className="text-xs text-fg-muted">{howTo}</p>}
    </div>
  )
}

import { useState } from "react"
import type { AfterUnpacking } from "../../bindings/AfterUnpacking"
import type { UnpackSettings } from "../../bindings/UnpackSettings"
import { Card } from "../../components/Card"
import { Checkbox, Field, SelectInput } from "../../components/Field"
import { Section } from "../../components/Section"
import { StatusMessage } from "../../components/StatusMessage"
import { classes, INPUT_CLASSES } from "../../components/styles"
import { useSaveDownloadOptions } from "../../queries"
import { strings } from "../../strings"

const text = strings.settings

/** What can become of an archive once unpacked, in the order shown. */
const AFTER_CHOICES: readonly AfterUnpacking[] = ["recycleBin", "delete", "keep"]

/** As the core has it before Settings is read (`UnpackSettings::default`). */
const DEFAULT_UNPACK: UnpackSettings = { enabled: true, after: "recycleBin", passwords: [] }

/** Settings → Archives: unpacking the archives downloads bring, what becomes of them after, and the passwords tried on
 * one that has a password. */
export function ArchivesSection() {
  const { options, save, error } = useSaveDownloadOptions()
  const unpack = options?.unpack ?? DEFAULT_UNPACK
  // The passwords are saved when the field is left, not at every key; the core drops blank lines and repeats.
  const [passwords, setPasswords] = useState<string | null>(null)
  const [saved, setSaved] = useState(false)
  const shownPasswords = passwords ?? unpack.passwords.join("\n")

  async function savePasswords() {
    if (passwords === null) return
    if (await save({ unpack: { ...unpack, passwords: passwords.split(/\r?\n/) } })) {
      setPasswords(null)
      setSaved(true)
    }
  }

  return (
    <Section title={text.archives}>
      <Card className="flex flex-col gap-4 p-4">
        <div className="flex flex-col gap-1">
          <Checkbox
            label={text.unpackArchives}
            checked={unpack.enabled}
            disabled={!options}
            onChange={(enabled) => void save({ unpack: { ...unpack, enabled } })}
          />
          <p className="pl-6 text-xs text-fg-muted">{text.unpackArchivesHint}</p>
        </div>
        <Field label={text.afterUnpacking} hint={text.afterUnpackingHint} htmlFor="settings-after-unpacking">
          <SelectInput
            id="settings-after-unpacking"
            className="self-start"
            value={unpack.after}
            disabled={!options || !unpack.enabled}
            onChange={(event) => void save({ unpack: { ...unpack, after: event.target.value as AfterUnpacking } })}
          >
            {AFTER_CHOICES.map((choice) => (
              <option key={choice} value={choice}>
                {text.afterUnpackingChoices[choice]}
              </option>
            ))}
          </SelectInput>
        </Field>
        <Field label={text.archivePasswords} hint={text.archivePasswordsHint} htmlFor="settings-archive-passwords">
          <textarea
            id="settings-archive-passwords"
            rows={4}
            spellCheck={false}
            autoComplete="off"
            className={classes(INPUT_CLASSES, "h-auto resize-y py-1.5 font-mono")}
            placeholder={text.archivePasswordsPlaceholder}
            value={shownPasswords}
            disabled={!options}
            onChange={(event) => {
              setSaved(false)
              setPasswords(event.target.value)
            }}
            onBlur={() => void savePasswords()}
          />
        </Field>
        {saved && <StatusMessage tone="success" message={text.archivePasswordsSaved(unpack.passwords.length)} />}
        {error && <StatusMessage tone="danger" {...error} />}
      </Card>
    </Section>
  )
}

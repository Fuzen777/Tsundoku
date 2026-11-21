import type { AfterUnpacking } from "../../bindings/AfterUnpacking"
import type { UnpackSettings } from "../../bindings/UnpackSettings"
import { UNPACK_PLACES } from "../../archives"
import { Card } from "../../components/Card"
import { Checkbox, Field } from "../../components/Field"
import { HelpMenu } from "../../components/HelpMenu"
import { Section } from "../../components/Section"
import { SegmentedControl } from "../../components/SegmentedControl"
import { StatusMessage } from "../../components/StatusMessage"
import { useSaveDownloadOptions } from "../../queries"
import { strings } from "../../strings"

const text = strings.settings

const PLACE_CHOICES = UNPACK_PLACES.map((place) => ({ value: place, label: text.unpackPlaceChoices[place] }))

/** What can become of an archive once unpacked, in the order shown. */
const AFTER_CHOICES: readonly { value: AfterUnpacking; label: string }[] = (
  ["recycleBin", "delete", "keep"] as const
).map((after) => ({ value: after, label: text.afterUnpackingChoices[after] }))

/** As the core has it before Settings is read (`UnpackSettings::default`). */
const DEFAULT_UNPACK: UnpackSettings = { enabled: true, place: "smart", after: "recycleBin", passwords: [] }

/** Settings → Archives: unpacking the archives downloads bring, where what they hold goes and what becomes of them
 * after. What the choices do is in its (?), How Unpacking Works. The passwords tried on an archive that has one are
 * Settings → Passwords', which links share. */
export function ArchivesSection() {
  const { options, save, error } = useSaveDownloadOptions()
  const unpack = options?.unpack ?? DEFAULT_UNPACK

  return (
    <Section
      title={text.archives}
      actions={
        <HelpMenu
          label={text.archivesHelp.label}
          topics={[
            { title: text.unpackPlace, paragraphs: text.archivesHelp.unpackPlace },
            { title: text.afterUnpacking, paragraphs: text.archivesHelp.afterUnpacking },
          ]}
        />
      }
    >
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
        <Field label={text.unpackPlace}>
          <SegmentedControl
            label={text.unpackPlace}
            options={PLACE_CHOICES}
            value={unpack.place}
            disabled={!options || !unpack.enabled}
            onChange={(place) => void save({ unpack: { ...unpack, place } })}
          />
        </Field>
        <Field label={text.afterUnpacking}>
          <SegmentedControl
            label={text.afterUnpacking}
            options={AFTER_CHOICES}
            value={unpack.after}
            disabled={!options || !unpack.enabled}
            onChange={(after) => void save({ unpack: { ...unpack, after } })}
          />
        </Field>
        <p className="text-xs text-fg-muted">{text.archivePasswordsElsewhere}</p>
        {error && <StatusMessage tone="danger" {...error} />}
      </Card>
    </Section>
  )
}

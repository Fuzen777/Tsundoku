import { Card } from "../../components/Card"
import { Field } from "../../components/Field"
import { HelpMenu } from "../../components/HelpMenu"
import { Section } from "../../components/Section"
import { SegmentedControl } from "../../components/SegmentedControl"
import { StatusMessage } from "../../components/StatusMessage"
import { useSaveWatchedOptions } from "../../queries"
import { strings } from "../../strings"
import { NEW_ITEMS_STARTS } from "../../watchedSources"

const text = strings.settings
const watchedText = strings.watched

/** Settings → Watched: whether what checks find starts ticked on every watched source without a choice of its own
 * (Edit on its card). What the choices do is in its (?), How New Items Start. */
export function WatchedSection() {
  const { options, save, error } = useSaveWatchedOptions()

  return (
    <Section
      title={text.watched}
      actions={
        <HelpMenu
          label={watchedText.newItemsHelp.label}
          topics={[{ title: watchedText.newItemsStart, paragraphs: watchedText.newItemsHelp.paragraphs }]}
        />
      }
    >
      <Card className="flex flex-col gap-4 p-4">
        <Field label={watchedText.newItemsStart} hint={text.newItemsStartHint}>
          <SegmentedControl
            label={watchedText.newItemsStart}
            options={NEW_ITEMS_STARTS.map((start) => ({ value: start, label: watchedText.newItemsStarts[start] }))}
            value={options?.newItemsStart ?? "unticked"}
            disabled={!options}
            onChange={(newItemsStart) => void save({ newItemsStart })}
          />
        </Field>
        {error && <StatusMessage tone="danger" {...error} />}
      </Card>
    </Section>
  )
}

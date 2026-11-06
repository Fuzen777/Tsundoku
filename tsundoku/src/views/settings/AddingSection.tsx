import { Card } from "../../components/Card"
import { Checkbox } from "../../components/Field"
import { Section } from "../../components/Section"
import { StatusMessage } from "../../components/StatusMessage"
import { useSaveAddingOptions } from "../../queries"
import { strings } from "../../strings"

const text = strings.settings

/** Settings → Adding Links: the clipboard watcher, and Download Right Away for what it catches. */
export function AddingSection() {
  const { options, save, error } = useSaveAddingOptions()
  const watching = options?.watchClipboard ?? false

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
        <div className="flex flex-col gap-1">
          <Checkbox
            label={text.downloadRightAway}
            checked={options?.downloadRightAway ?? false}
            disabled={!options || !watching}
            onChange={(checked) => void save({ downloadRightAway: checked })}
          />
          <p className="pl-6 text-xs text-fg-muted">{text.downloadRightAwayHint}</p>
        </div>
        {error && <StatusMessage tone="danger" {...error} />}
      </Card>
    </Section>
  )
}

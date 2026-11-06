import { Card } from "../../components/Card"
import { Checkbox } from "../../components/Field"
import { Section } from "../../components/Section"
import { StatusMessage } from "../../components/StatusMessage"
import { useSaveBackgroundOptions } from "../../queries"
import { strings } from "../../strings"

const text = strings.settings

/** Settings → Running in the Background: keeping Tsundoku in the tray when its window closes, and starting it with
 * Windows. */
export function BackgroundSection() {
  const { options, save, error } = useSaveBackgroundOptions()

  return (
    <Section title={text.background}>
      <Card className="flex flex-col gap-4 p-4">
        <div className="flex flex-col gap-1">
          <Checkbox
            label={text.keepInTray}
            checked={options?.keepInTray ?? false}
            disabled={!options}
            onChange={(checked) => void save({ keepInTray: checked })}
          />
          <p className="pl-6 text-xs text-fg-muted">{text.keepInTrayHint}</p>
        </div>
        <div className="flex flex-col gap-1">
          <Checkbox
            label={text.startWithWindows}
            checked={options?.startWithWindows ?? false}
            disabled={!options}
            onChange={(checked) => void save({ startWithWindows: checked })}
          />
          <p className="pl-6 text-xs text-fg-muted">{text.startWithWindowsHint}</p>
        </div>
        {error && <StatusMessage tone="danger" {...error} />}
      </Card>
    </Section>
  )
}

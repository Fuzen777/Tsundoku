import type { Theme } from "../../bindings/Theme"
import { Card } from "../../components/Card"
import { Field } from "../../components/Field"
import { Section } from "../../components/Section"
import { SegmentedControl } from "../../components/SegmentedControl"
import { StatusMessage } from "../../components/StatusMessage"
import { useSaveAppearanceOptions } from "../../queries"
import { strings } from "../../strings"

const text = strings.settings

const THEME_OPTIONS = [
  { value: "auto", label: text.themes.auto },
  { value: "light", label: text.themes.light },
  { value: "dark", label: text.themes.dark },
] as const satisfies readonly { value: Theme; label: string }[]

/** Settings → Appearance: light, dark, or Windows' theme, taken at once. */
export function AppearanceSection() {
  const { options, save, error } = useSaveAppearanceOptions()

  return (
    <Section title={text.appearance}>
      <Card className="flex flex-col gap-4 p-4">
        <Field label={text.theme} hint={text.themeHint}>
          <SegmentedControl
            label={text.theme}
            options={THEME_OPTIONS}
            value={options?.theme ?? "auto"}
            disabled={!options}
            onChange={(theme) => void save({ theme })}
          />
        </Field>
        {error && <StatusMessage tone="danger" {...error} />}
      </Card>
    </Section>
  )
}

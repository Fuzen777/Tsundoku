import { useState } from "react"
import { Check, Pipette } from "lucide-react"
import type { Accent } from "../../bindings/Accent"
import type { AppearanceSettings } from "../../bindings/AppearanceSettings"
import type { Theme } from "../../bindings/Theme"
import { pickColor } from "../../api"
import { Card } from "../../components/Card"
import { Field } from "../../components/Field"
import { Section } from "../../components/Section"
import { SegmentedControl } from "../../components/SegmentedControl"
import { StatusMessage } from "../../components/StatusMessage"
import { Tooltip } from "../../components/Tooltip"
import { useSaveAppearanceOptions } from "../../queries"
import { strings } from "../../strings"

const text = strings.settings

const THEME_OPTIONS = [
  { value: "auto", label: text.themes.auto },
  { value: "light", label: text.themes.light },
  { value: "dark", label: text.themes.dark },
] as const satisfies readonly { value: Theme; label: string }[]

const ACCENTS: readonly Accent[] = ["indigo", "teal", "green", "amber", "rose", "purple"]

const SWATCH_CLASSES =
  "flex size-7 items-center justify-center rounded-full bg-accent-fill text-on-accent transition-transform hover:scale-110 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent disabled:opacity-40"
const SELECTED_SWATCH_CLASSES = "outline-2 outline-offset-2 outline-fg-secondary"

/** Settings → Appearance: light, dark, or Windows' theme, and the accent color, each taken at once. */
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
        <Field label={text.accent} hint={text.accentHint}>
          <div role="radiogroup" aria-label={text.accent} className="flex flex-wrap gap-2">
            {ACCENTS.map((accent) => {
              const selected = (options?.accent ?? "indigo") === accent
              return (
                <Tooltip key={accent} label={text.accents[accent]}>
                  <button
                    type="button"
                    role="radio"
                    aria-checked={selected}
                    aria-label={text.accents[accent]}
                    data-accent={accent}
                    disabled={!options}
                    onClick={() => {
                      if (!selected) void save({ accent })
                    }}
                    className={`${SWATCH_CLASSES} ${selected ? SELECTED_SWATCH_CLASSES : ""}`}
                  >
                    {selected && <Check size={14} aria-hidden />}
                  </button>
                </Tooltip>
              )
            })}
            <CustomAccentSwatch options={options} onPick={(customAccent) => void save({ accent: "custom", customAccent })} />
          </div>
        </Field>
        {error && <StatusMessage tone="danger" {...error} />}
      </Card>
    </Section>
  )
}

/**
 * The last swatch: any color, from Windows' color picker. It shows the color picked last; picking one, or clicking it
 * once it's the accent, opens the picker on that color, and the color is saved once the picker closes on it.
 */
function CustomAccentSwatch({
  options,
  onPick,
}: {
  options: AppearanceSettings | undefined
  onPick: (color: string) => void
}) {
  const selected = options?.accent === "custom"
  const [picking, setPicking] = useState(false)

  const openPicker = async () => {
    if (!options) return
    setPicking(true)
    try {
      const color = await pickColor(options.customAccent)
      if (color) onPick(color)
    } finally {
      setPicking(false)
    }
  }

  return (
    <Tooltip label={text.accents.custom}>
      <button
        type="button"
        role="radio"
        aria-checked={selected}
        aria-label={text.accents.custom}
        data-accent="custom"
        disabled={!options || picking}
        onClick={() => void openPicker()}
        className={`${SWATCH_CLASSES} ${selected ? SELECTED_SWATCH_CLASSES : ""}`}
      >
        {selected ? <Check size={14} aria-hidden /> : <Pipette size={14} aria-hidden />}
      </button>
    </Tooltip>
  )
}

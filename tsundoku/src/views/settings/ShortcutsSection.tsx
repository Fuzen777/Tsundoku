import { Card } from "../../components/Card"
import { Section } from "../../components/Section"
import { mainNavItems, settingsNavItem } from "../../navigation"
import { useSaveAddingOptions } from "../../queries"
import { SHORTCUT_VIEWS } from "../../shortcuts"
import { strings } from "../../strings"

const keys = strings.shortcuts
const actions = strings.settings.shortcutActions

/** Settings → Keyboard Shortcuts: the window's shortcuts (shortcuts.ts), and the clipboard shortcut once one is picked. */
export function ShortcutsSection() {
  const { options } = useSaveAddingOptions()
  const clipboardShortcut = options?.clipboardShortcut ?? null
  const viewNames = [...mainNavItems, settingsNavItem].map((item) => item.label)

  const rows: [string, string][] = [
    [keys.paste, actions.paste],
    [keys.focusAddBar, actions.focusAddBar],
    [keys.views(SHORTCUT_VIEWS.length), actions.views(viewNames)],
    [keys.delete, actions.delete],
  ]
  if (clipboardShortcut) {
    rows.push([strings.settings.shortcutKeys[clipboardShortcut], actions.clipboardShortcut])
  }

  return (
    <Section title={strings.settings.keyboardShortcuts}>
      <Card className="p-4">
        <dl className="grid grid-cols-[max-content_minmax(0,1fr)] items-baseline gap-x-4 gap-y-2.5">
          {rows.map(([shortcut, action]) => (
            <div key={shortcut} className="contents">
              <dt>
                <kbd className="rounded border border-line bg-input px-1.5 py-0.5 font-mono text-xs">{shortcut}</kbd>
              </dt>
              <dd className="text-fg-secondary">{action}</dd>
            </div>
          ))}
        </dl>
      </Card>
    </Section>
  )
}

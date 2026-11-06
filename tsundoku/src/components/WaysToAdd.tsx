import { useState } from "react"
import { Bookmark, Check, ClipboardCheck, ClipboardPaste, FileInput, Keyboard, MousePointerClick } from "lucide-react"
import type { LucideIcon } from "lucide-react"
import { clipboardText } from "../api"
import { chooseLinkFiles } from "../linkFiles"
import { SETTINGS_PLACES, useAppActions } from "../navigation"
import { useSaveAddingOptions } from "../queries"
import { strings } from "../strings"
import { Button } from "./Button"
import { Card } from "./Card"

export type WayToAdd = "paste" | "drop" | "watch" | "shortcut" | "bookmark" | "sendButtons"

interface WayRow {
  icon: LucideIcon
  label: string
  state: string
  /** Its one button, or none when there's nothing to do, such as Watch Clipboard once it's on. */
  action: { label: string; run: () => void | Promise<void> } | null
}

const text = strings.waysToAdd

/**
 * The ways to add links, in the order given, each with where it stands and one button: Paste and Choose File… add
 * at once, Turn On turns the clipboard watcher on, and Set Up or Change opens Settings → Adding Links at its place.
 * The empty Queue and Pending show it.
 */
export function WaysToAdd({ ways }: { ways: readonly WayToAdd[] }) {
  const { navigate, addText, addFileLinks } = useAppActions()
  const { options, save } = useSaveAddingOptions()
  const [busy, setBusy] = useState<WayToAdd | null>(null)
  const watching = options?.watchClipboard ?? false
  const shortcut = options?.clipboardShortcut ?? null

  const rows: Record<WayToAdd, WayRow> = {
    paste: {
      icon: ClipboardPaste,
      label: text.paste,
      state: text.pasteState,
      action: {
        label: text.pasteAction,
        run: async () => {
          const copied = await clipboardText()
          if (copied?.trim()) addText(copied)
        },
      },
    },
    drop: {
      icon: FileInput,
      label: text.drop,
      state: text.dropState,
      action: {
        label: text.dropAction,
        run: async () => {
          const read = await chooseLinkFiles()
          if (read) addFileLinks(read)
        },
      },
    },
    watch: {
      icon: ClipboardCheck,
      label: text.watch,
      state: watching ? text.watchOn(options?.downloadRightAway ?? false) : text.watchOff,
      action: watching ? null : { label: text.watchAction, run: () => save({ watchClipboard: true }) },
    },
    shortcut: {
      icon: Keyboard,
      label: text.shortcut,
      state: shortcut ? text.shortcutState(strings.settings.shortcutKeys[shortcut]) : text.shortcutNone,
      action: {
        label: shortcut ? text.change : text.setUp,
        run: () => navigate("settings", SETTINGS_PLACES.clipboardShortcut),
      },
    },
    bookmark: {
      icon: Bookmark,
      label: text.bookmark,
      state: text.bookmarkState,
      action: { label: text.setUp, run: () => navigate("settings", SETTINGS_PLACES.sendPage) },
    },
    sendButtons: {
      icon: MousePointerClick,
      label: text.sendButtons,
      state: text.sendButtonsState,
      action: { label: text.setUp, run: () => navigate("settings", SETTINGS_PLACES.sendButtons) },
    },
  }

  async function run(way: WayToAdd, action: () => void | Promise<void>) {
    setBusy(way)
    try {
      await action()
    } finally {
      setBusy(null)
    }
  }

  return (
    <Card className="w-full max-w-lg divide-y divide-line text-left">
      {ways.map((way) => {
        const { icon: Icon, label, state, action } = rows[way]
        return (
          <div key={way} className="flex items-center gap-3 px-4 py-2.5">
            <Icon size={16} className="shrink-0 text-fg-muted" />
            <div className="min-w-0 flex-1">
              <p className="font-medium">{label}</p>
              <p className="text-xs text-fg-secondary">{state}</p>
            </div>
            {action ? (
              <Button
                size="sm"
                loading={busy === way}
                disabled={busy !== null && busy !== way}
                onClick={() => void run(way, action.run)}
              >
                {action.label}
              </Button>
            ) : (
              <span className="flex shrink-0 items-center gap-1 text-xs text-success">
                <Check size={14} />
                {text.on}
              </span>
            )}
          </div>
        )
      })}
    </Card>
  )
}

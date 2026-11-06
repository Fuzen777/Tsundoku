import { createContext, useContext } from "react"
import { FolderTree, History, Inbox, ListVideo, Settings, SlidersHorizontal, type LucideIcon } from "lucide-react"
import { strings } from "./strings"

export type ViewId = "queue" | "pending" | "history" | "presets" | "rules" | "settings"

export interface NavItem {
  id: ViewId
  label: string
  icon: LucideIcon
}

/** The views at the top of the sidebar. */
export const mainNavItems: readonly NavItem[] = [
  { id: "queue", label: strings.nav.queue, icon: ListVideo },
  { id: "pending", label: strings.nav.pending, icon: Inbox },
  { id: "history", label: strings.nav.history, icon: History },
  { id: "presets", label: strings.nav.presets, icon: SlidersHorizontal },
  { id: "rules", label: strings.nav.rules, icon: FolderTree },
]

/** Settings sits apart, at the bottom of the sidebar. */
export const settingsNavItem: NavItem = { id: "settings", label: strings.nav.settings, icon: Settings }

/** Places in Settings that another view sends the user to, as the IDs of their elements. */
export const SETTINGS_PLACES = {
  clipboardShortcut: "settings-clipboard-shortcut",
  sendPage: "settings-send-page",
  sendButtons: "settings-send-buttons",
} as const

/** What any view can ask of the app around it. */
export interface AppActions {
  navigate: (view: ViewId) => void
  /** Puts a link in the add bar and fetches its preview, as a dropped link is. */
  showLink: (link: string) => void
}

export const AppActionsContext = createContext<AppActions>({ navigate: () => {}, showLink: () => {} })

export function useAppActions(): AppActions {
  return useContext(AppActionsContext)
}


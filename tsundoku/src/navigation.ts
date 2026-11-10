import { createContext, useContext } from "react"
import { Eye, FolderTree, History, Inbox, ListVideo, Settings, SlidersHorizontal, type LucideIcon } from "lucide-react"
import type { LinkFilesRead } from "./linkFiles"
import { strings } from "./strings"

export type ViewId = "queue" | "pending" | "watched" | "history" | "presets" | "rules" | "settings"

export interface NavItem {
  id: ViewId
  label: string
  icon: LucideIcon
}

/** The views at the top of the sidebar. */
export const mainNavItems: readonly NavItem[] = [
  { id: "queue", label: strings.nav.queue, icon: ListVideo },
  { id: "pending", label: strings.nav.pending, icon: Inbox },
  { id: "watched", label: strings.nav.watched, icon: Eye },
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

export type SettingsPlace = (typeof SETTINGS_PLACES)[keyof typeof SETTINGS_PLACES]

/** A place Settings was asked to open at: a new object each time, so asking again scrolls there again. */
export interface SettingsTarget {
  place: SettingsPlace
}

/** What any view can ask of the app around it. */
export interface AppActions {
  /** Opens a view; Settings at a place in it, when one is given. */
  navigate: (view: ViewId, place?: SettingsPlace) => void
  /** Puts a link in the add bar and fetches its preview, as a dropped link is. */
  showLink: (link: string) => void
  /** Adds text as if pasted in the add bar and submitted: one link's preview, or several links to Pending. */
  addText: (text: string) => void
  /** Adds links read from files to Pending, as a dropped file's are (asking first for many), or says why there were
   * none. */
  addFileLinks: (read: LinkFilesRead) => void
}

export const AppActionsContext = createContext<AppActions>({
  navigate: () => {},
  showLink: () => {},
  addText: () => {},
  addFileLinks: () => {},
})

export function useAppActions(): AppActions {
  return useContext(AppActionsContext)
}


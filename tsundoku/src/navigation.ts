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

/** Settings sits at the bottom of the sidebar, and moves up under the views while it's open, its sections under it. */
export const settingsNavItem: NavItem = {
  id: "settings",
  label: strings.nav.settings,
  icon: Settings,
}

/** Settings' sections in order, as the sidebar lists them under Settings: each one's heading, or a shorter name. */
export const SETTINGS_SECTIONS = [
  { id: "settings-downloads", title: strings.settings.downloads },
  { id: "settings-archives", title: strings.settings.archives },
  { id: "settings-passwords", title: strings.settings.passwords },
  { id: "settings-adding", title: strings.settings.adding },
  { id: "settings-watched", title: strings.settings.watched },
  { id: "settings-background", title: strings.settings.backgroundShort },
  { id: "settings-appearance", title: strings.settings.appearance },
  { id: "settings-shortcuts", title: strings.settings.keyboardShortcuts },
  { id: "settings-posts", title: strings.settings.posts },
  { id: "settings-pacing", title: strings.settings.pacing },
  { id: "settings-cookies", title: strings.settings.cookies },
  { id: "settings-sign-ins", title: strings.settings.signIns.title },
  { id: "settings-sites", title: strings.settings.sites.title },
  { id: "settings-tools", title: strings.settings.tools },
  { id: "settings-about", title: strings.settings.about },
] as const

export type SettingsSectionId = (typeof SETTINGS_SECTIONS)[number]["id"]

/** Places in Settings that another view sends the user to, as the IDs of their elements. */
export const SETTINGS_PLACES = {
  clipboardShortcut: "settings-clipboard-shortcut",
  sendPage: "settings-send-page",
  sendButtons: "settings-send-buttons",
} as const

export type SettingsPlace = (typeof SETTINGS_PLACES)[keyof typeof SETTINGS_PLACES] | SettingsSectionId

/** Whether a place is one of Settings' sections, which opens at its top, rather than a control in one. */
export function isSettingsSection(place: SettingsPlace): place is SettingsSectionId {
  return SETTINGS_SECTIONS.some((section) => section.id === place)
}

/** A place Settings was asked to open at: a new object each time, so asking again scrolls there again. */
export interface SettingsTarget {
  place: SettingsPlace
}

/** What any view can ask of the app around it. */
export interface AppActions {
  /** Opens a view; Settings at a place in it (a section or a control), when one is given. */
  navigate: (view: ViewId, place?: SettingsPlace) => void
  /** Fetches a link's preview, as a dropped link's is, and keeps it as the add bar's field's text. */
  showLink: (link: string) => void
  /** Shows a pending link that lists several items in the preview card, to pick from: once what's picked joins the
   * queue, or it's watched, the pending link leaves Pending. */
  pickFromPending: (id: number, link: string) => void
  /** Adds what the clipboard holds, as the add bar's Paste Link does: one link's preview, or several links to
   * Pending. With no link there, the add bar says so. */
  pasteFromClipboard: () => Promise<void>
  /** Adds links read from files to Pending, as a dropped file's are (asking first for many), or says why there were
   * none. */
  addFileLinks: (read: LinkFilesRead) => void
  /** Opens the history at a watched source's downloads: its Source filter set, the others cleared. */
  showWatchedDownloads: (sourceId: number) => void
  /** Opens the What's New card above the view on the running version's changes, with the earlier versions' to
   * pick from. */
  showWhatsNew: () => void
}

/** What the history opens showing, when another view sends the user there: a new object each time, so the history
 * starts again from it. */
export interface HistoryTarget {
  watchedSource: number
}

export const AppActionsContext = createContext<AppActions>({
  navigate: () => {},
  showLink: () => {},
  pickFromPending: () => {},
  pasteFromClipboard: () => Promise.resolve(),
  addFileLinks: () => {},
  showWatchedDownloads: () => {},
  showWhatsNew: () => {},
})

export function useAppActions(): AppActions {
  return useContext(AppActionsContext)
}

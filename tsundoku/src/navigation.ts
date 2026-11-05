import { createContext, useContext } from "react"
import { History, ListVideo, Settings, SlidersHorizontal, type LucideIcon } from "lucide-react"
import { strings } from "./strings"

export type ViewId = "queue" | "history" | "presets" | "settings"

export interface NavItem {
  id: ViewId
  label: string
  icon: LucideIcon
}

/** The views at the top of the sidebar. */
export const mainNavItems: readonly NavItem[] = [
  { id: "queue", label: strings.nav.queue, icon: ListVideo },
  { id: "history", label: strings.nav.history, icon: History },
  { id: "presets", label: strings.nav.presets, icon: SlidersHorizontal },
]

/** Settings sits apart, at the bottom of the sidebar. */
export const settingsNavItem: NavItem = { id: "settings", label: strings.nav.settings, icon: Settings }

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


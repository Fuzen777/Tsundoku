import { useEffect, useState } from "react"
import { showMainWindow } from "./api"
import { AddBar } from "./components/AddBar"
import { Sidebar } from "./components/Sidebar"
import type { ViewId } from "./navigation"
import { strings } from "./strings"
import { HistoryView } from "./views/HistoryView"
import { PresetsView } from "./views/PresetsView"
import { QueueView } from "./views/QueueView"
import { SettingsView } from "./views/SettingsView"

export function App() {
  const [view, setView] = useState<ViewId>("queue")

  useEffect(() => {
    // After the first frame is painted, so the hidden window never shows blank.
    const frame = requestAnimationFrame(() => void showMainWindow())
    return () => cancelAnimationFrame(frame)
  }, [])

  return (
    <div className="grid h-full grid-cols-[13rem_minmax(0,1fr)] grid-rows-[auto_minmax(0,1fr)]">
      <div className="flex items-baseline gap-2 px-5 py-4">
        <span className="text-base font-semibold">{strings.appName}</span>
        <span className="text-xs text-fg-muted" lang="ja">
          {strings.appNameJapanese}
        </span>
      </div>
      <div className="flex items-center py-3 pr-4">
        <AddBar />
      </div>
      <Sidebar current={view} onSelect={setView} />
      <main className="min-h-0 rounded-tl-lg border-t border-l border-line bg-surface">
        <CurrentView view={view} />
      </main>
    </div>
  )
}

function CurrentView({ view }: { view: ViewId }) {
  switch (view) {
    case "queue":
      return <QueueView />
    case "history":
      return <HistoryView />
    case "presets":
      return <PresetsView />
    case "settings":
      return <SettingsView />
  }
}

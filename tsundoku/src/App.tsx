import { useCallback, useEffect, useState } from "react"
import { showMainWindow } from "./api"
import { AddBar } from "./components/AddBar"
import { AppTitle } from "./components/AppTitle"
import { PreviewCard } from "./components/PreviewCard"
import { Sidebar } from "./components/Sidebar"
import type { ViewId } from "./navigation"
import { useToolsNeedSetup } from "./queries"
import { useLinkDrop, usePreview } from "./usePreview"
import { HistoryView } from "./views/HistoryView"
import { PresetsView } from "./views/PresetsView"
import { QueueView } from "./views/QueueView"
import { SettingsView } from "./views/SettingsView"
import { SetupView } from "./views/SetupView"

export function App() {
  const [view, setView] = useState<ViewId>("queue")
  const [link, setLink] = useState("")
  const { data: needsSetup } = useToolsNeedSetup()
  const preview = usePreview()
  const { show: showPreview } = preview

  useEffect(() => {
    // After the first frame is painted, so the hidden window never shows blank.
    const frame = requestAnimationFrame(() => void showMainWindow())
    return () => cancelAnimationFrame(frame)
  }, [])

  // A dropped link goes in the add bar and is fetched right away.
  const showDroppedLink = useCallback(
    (dropped: string) => {
      setLink(dropped)
      void showPreview(dropped)
    },
    [showPreview],
  )
  useLinkDrop(showDroppedLink)

  if (needsSetup) return <SetupView />

  return (
    <div className="grid h-full grid-cols-[13rem_minmax(0,1fr)] grid-rows-[auto_auto_minmax(0,1fr)]">
      <AppTitle />
      <div className="flex items-center py-3 pr-4">
        <AddBar link={link} onLinkChange={setLink} onSubmit={() => void showPreview(link)} />
      </div>
      <Sidebar className="col-start-1 row-span-2 row-start-2" current={view} onSelect={setView} />
      {preview.state.stage !== "idle" && (
        <div className="col-start-2 row-start-2 pr-4 pb-3">
          <PreviewCard state={preview.state} onClose={preview.close} />
        </div>
      )}
      <main className="col-start-2 row-start-3 min-h-0 rounded-tl-lg border-t border-l border-line bg-surface">
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

import { useCallback, useEffect, useMemo, useState } from "react"
import { showMainWindow } from "./api"
import { AddBar } from "./components/AddBar"
import { AppTitle } from "./components/AppTitle"
import { PreviewCard } from "./components/PreviewCard"
import { Sidebar } from "./components/Sidebar"
import { UpdateBanner } from "./components/UpdateBanner"
import { AppActionsContext, type ViewId } from "./navigation"
import { useQueue, useToolsNeedSetup } from "./queries"
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
  const { data: queue = [] } = useQueue()
  const unfinished = queue.filter((item) => item.state !== "done").length

  useEffect(() => {
    // After the first frame is painted, so the hidden window never shows blank.
    const frame = requestAnimationFrame(() => void showMainWindow())
    return () => cancelAnimationFrame(frame)
  }, [])

  // A dropped link, or Download Again in the history, goes in the add bar and is fetched right away.
  const showLink = useCallback(
    (shown: string) => {
      setLink(shown)
      void showPreview(shown)
    },
    [showPreview],
  )
  useLinkDrop(showLink)
  const actions = useMemo(() => ({ navigate: setView, showLink }), [showLink])

  if (needsSetup) return <SetupView />

  return (
    <AppActionsContext value={actions}>
      <div className="grid h-full grid-cols-[13rem_minmax(0,1fr)] grid-rows-[auto_auto_minmax(0,1fr)]">
        <AppTitle />
        <div className="flex items-center py-3 pr-4">
          <AddBar link={link} onLinkChange={setLink} onSubmit={() => void showPreview(link)} />
        </div>
        <Sidebar
          className="col-start-1 row-span-2 row-start-2"
          current={view}
          counts={{ queue: unfinished }}
          onSelect={setView}
        />
        {/* Hidden while it has nothing to show, so the row takes no room. */}
        <div className="col-start-2 row-start-2 flex flex-col gap-3 pr-4 pb-3 empty:hidden">
          <UpdateBanner />
          {preview.state.stage !== "idle" && (
            <PreviewCard
              state={preview.state}
              onClose={preview.close}
              onAdded={() => {
                preview.close()
                setLink("")
              }}
            />
          )}
        </div>
        <main className="col-start-2 row-start-3 min-h-0 rounded-tl-lg border-t border-l border-line bg-surface">
          <CurrentView view={view} />
        </main>
      </div>
    </AppActionsContext>
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

import { useCallback, useEffect, useMemo, useState } from "react"
import { addPendingLinks, linksInText, showMainWindow } from "./api"
import { AddBar } from "./components/AddBar"
import { AppTitle } from "./components/AppTitle"
import { Button } from "./components/Button"
import { Card } from "./components/Card"
import { PreviewCard } from "./components/PreviewCard"
import { Sidebar } from "./components/Sidebar"
import { StatusMessage } from "./components/StatusMessage"
import { UpdateBanner } from "./components/UpdateBanner"
import { describeDownloadError, type ErrorDescription } from "./errors"
import { AppActionsContext, type ViewId } from "./navigation"
import { usePendingLinks, useQueue, useToolsNeedSetup } from "./queries"
import { strings } from "./strings"
import { useLinkDrop, usePreview } from "./usePreview"
import { HistoryView } from "./views/HistoryView"
import { PendingView } from "./views/PendingView"
import { PresetsView } from "./views/PresetsView"
import { QueueView } from "./views/QueueView"
import { RulesView } from "./views/RulesView"
import { SettingsView } from "./views/SettingsView"
import { SetupView } from "./views/SetupView"

export function App() {
  const [view, setView] = useState<ViewId>("queue")
  const [link, setLink] = useState("")
  const [addError, setAddError] = useState<ErrorDescription | null>(null)
  const { data: needsSetup } = useToolsNeedSetup()
  const preview = usePreview()
  const { show: showPreview, close: closePreview } = preview
  const { data: queue = [] } = useQueue()
  const { data: pending = [] } = usePendingLinks()
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
  // Text pasted or dropped with one link in it is previewed as before. Several links wait in Pending instead, where
  // they're read one at a time.
  const addText = useCallback(
    async (text: string) => {
      setAddError(null)
      const links = await linksInText(text)
      if (links.length < 2) {
        showLink(text.trim())
        return
      }
      try {
        await addPendingLinks(links)
        setLink("")
        closePreview()
        setView("pending")
      } catch (error) {
        setAddError(describeDownloadError(error))
      }
    },
    [showLink, closePreview],
  )
  const dropText = useCallback((text: string) => void addText(text), [addText])
  useLinkDrop(dropText)
  const actions = useMemo(() => ({ navigate: setView, showLink }), [showLink])

  if (needsSetup) return <SetupView />

  return (
    <AppActionsContext value={actions}>
      <div className="grid h-full grid-cols-[13rem_minmax(0,1fr)] grid-rows-[auto_auto_minmax(0,1fr)]">
        <AppTitle />
        <div className="flex items-center py-3 pr-4">
          <AddBar link={link} onLinkChange={setLink} onSubmit={() => void addText(link)} />
        </div>
        <Sidebar
          className="col-start-1 row-span-2 row-start-2"
          current={view}
          counts={{ queue: unfinished, pending: pending.length }}
          onSelect={setView}
        />
        {/* Hidden while it has nothing to show, so the row takes no room. */}
        <div className="col-start-2 row-start-2 flex flex-col gap-3 pr-4 pb-3 empty:hidden">
          <UpdateBanner />
          {addError && (
            <Card className="flex items-start gap-3 p-4">
              <div className="min-w-0 flex-1">
                <StatusMessage tone="danger" {...addError} />
              </div>
              <Button size="sm" variant="ghost" onClick={() => setAddError(null)}>
                {strings.addBar.errors.dismiss}
              </Button>
            </Card>
          )}
          {preview.state.stage !== "idle" && (
            <PreviewCard
              state={preview.state}
              onClose={preview.close}
              onRetry={(retryLink) => void preview.show(retryLink)}
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
    case "pending":
      return <PendingView />
    case "history":
      return <HistoryView />
    case "presets":
      return <PresetsView />
    case "rules":
      return <RulesView />
    case "settings":
      return <SettingsView />
  }
}

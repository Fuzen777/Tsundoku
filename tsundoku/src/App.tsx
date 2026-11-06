import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { addPendingLinks, addPendingLinksFromFiles, confirmWithUser, linksInText, showMainWindow } from "./api"
import { AddBar } from "./components/AddBar"
import { AppTitle } from "./components/AppTitle"
import { Button } from "./components/Button"
import { Card } from "./components/Card"
import { PreviewCard } from "./components/PreviewCard"
import { Sidebar } from "./components/Sidebar"
import { StatusMessage } from "./components/StatusMessage"
import { UndoToastProvider } from "./components/UndoToast"
import { UpdateBanner } from "./components/UpdateBanner"
import { WhatsNewCard } from "./components/WhatsNewCard"
import { describeDownloadError, type ErrorDescription } from "./errors"
import { ASK_BEFORE_ADDING_OVER, checkingTime, type LinkFilesRead, readLinkFiles } from "./linkFiles"
import { AppActionsContext, type SettingsPlace, type SettingsTarget, type ViewId } from "./navigation"
import { shownPendingLinks } from "./pendingLinks"
import { useAppInfo, usePendingLinks, useQueue, useToolsNeedSetup } from "./queries"
import { useAppShortcuts, useWindowPaste } from "./shortcuts"
import { strings } from "./strings"
import { type Drop, useLinkDrop, usePreview } from "./usePreview"
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
  /** Why dropped files gave no links: a type Tsundoku doesn't read, or none in it. */
  const [fileProblems, setFileProblems] = useState<ErrorDescription[]>([])
  const { data: needsSetup } = useToolsNeedSetup()
  const preview = usePreview()
  const { show: showPreview, close: closePreview } = preview
  const { data: queue = [] } = useQueue()
  const { data: pending = [] } = usePendingLinks()
  const unfinished = queue.filter((item) => item.state !== "done").length

  const { data: appInfo } = useAppInfo()
  const startedInTray = appInfo?.startedInTray ?? false

  useEffect(() => {
    // Started by Windows with the tray on, the window waits there until it's opened.
    if (startedInTray) return
    // After the first frame is painted, so the hidden window never shows blank.
    const frame = requestAnimationFrame(() => void showMainWindow())
    return () => cancelAnimationFrame(frame)
  }, [startedInTray])

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
  const takeText = useCallback((text: string) => void addText(text), [addText])

  // A dropped file of links (a text file, a bookmarks export, an Internet shortcut) sends its links to Pending, where
  // they're checked as copied links are: those with nothing to download leave once read. Over 300 links, it asks
  // first. When a drop holds text too and none of its files is one Tsundoku reads, such as a linked image dragged
  // from a browser, the text goes in instead.
  const addFileLinks = useCallback(
    async (read: LinkFilesRead) => {
      setAddError(null)
      setFileProblems(read.problems)
      if (read.links.length === 0) return
      if (
        read.links.length > ASK_BEFORE_ADDING_OVER &&
        !(await confirmWithUser({
          title: strings.addBar.files.manyTitle(read.links.length),
          message: strings.addBar.files.manyMessage(read.sources, read.links.length, checkingTime(read.links.length)),
          confirmLabel: strings.addBar.files.addThem,
          cancelLabel: strings.dialogs.cancel,
        }))
      ) {
        return
      }
      try {
        await addPendingLinksFromFiles(read.links)
        setLink("")
        closePreview()
        setView("pending")
      } catch (error) {
        setAddError(describeDownloadError(error))
      }
    },
    [closePreview],
  )
  const addDrop = useCallback(
    async ({ text, files }: Drop) => {
      if (files.length > 0) {
        const read = await readLinkFiles(files)
        if (read.readable > 0 || !text) {
          await addFileLinks(read)
          return
        }
      }
      if (text) await addText(text)
    },
    [addText, addFileLinks],
  )
  const takeDrop = useCallback((drop: Drop) => void addDrop(drop), [addDrop])
  useLinkDrop(takeDrop)

  // Ctrl+V outside a text field adds what was copied as if pasted in the add bar and submitted; Ctrl+L goes to the
  // add bar, and Ctrl+1 to Ctrl+6 to the sidebar's views. Not during the first-run setup.
  const addBarInput = useRef<HTMLInputElement>(null)
  const shortcutsOn = needsSetup === false
  useWindowPaste(takeText, shortcutsOn)
  const focusAddBar = useCallback(() => {
    addBarInput.current?.focus()
    addBarInput.current?.select()
  }, [])
  useAppShortcuts({ focusAddBar, navigate: setView }, shortcutsOn)

  // Where Settings opens, when another view sends the user to a place in it. A new object each time, so going to the
  // same place again scrolls there again.
  const [settingsPlace, setSettingsPlace] = useState<SettingsTarget | null>(null)
  const navigate = useCallback((to: ViewId, place?: SettingsPlace) => {
    setView(to)
    setSettingsPlace(place ? { place } : null)
  }, [])
  const actions = useMemo(
    () => ({
      navigate,
      showLink,
      addText: takeText,
      addFileLinks: (read: LinkFilesRead) => void addFileLinks(read),
    }),
    [navigate, showLink, takeText, addFileLinks],
  )

  if (needsSetup) return <SetupView />

  return (
    <AppActionsContext value={actions}>
      <div className="grid h-full grid-cols-[13rem_minmax(0,1fr)] grid-rows-[auto_auto_minmax(0,1fr)]">
        <AppTitle />
        <div className="flex items-center py-3 pr-4">
          <AddBar
            link={link}
            onLinkChange={setLink}
            onSubmit={() => void addText(link)}
            inputRef={addBarInput}
          />
        </div>
        <Sidebar
          className="col-start-1 row-span-2 row-start-2"
          current={view}
          counts={{ queue: unfinished, pending: shownPendingLinks(pending).length }}
          onSelect={setView}
        />
        {/* Hidden while it has nothing to show, so the row takes no room. */}
        <div className="col-start-2 row-start-2 flex flex-col gap-3 pr-4 pb-3 empty:hidden">
          <UpdateBanner />
          <WhatsNewCard />
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
          {fileProblems.length > 0 && (
            <Card className="flex items-start gap-3 p-4">
              <div className="flex min-w-0 flex-1 flex-col gap-2">
                {fileProblems.map((problem) => (
                  <StatusMessage key={problem.message} tone="warning" {...problem} />
                ))}
              </div>
              <Button size="sm" variant="ghost" onClick={() => setFileProblems([])}>
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
        <main className="relative col-start-2 row-start-3 min-h-0 rounded-tl-lg border-t border-l border-line bg-surface">
          {/* Outside the views, so an offer to undo stays when another view is opened. */}
          <UndoToastProvider>
            <CurrentView view={view} settingsPlace={settingsPlace} />
          </UndoToastProvider>
        </main>
      </div>
    </AppActionsContext>
  )
}

function CurrentView({ view, settingsPlace }: { view: ViewId; settingsPlace: SettingsTarget | null }) {
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
      return <SettingsView place={settingsPlace} />
  }
}

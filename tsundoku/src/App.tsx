import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { PanelLeftClose, PanelLeftOpen } from "lucide-react"
import {
  addLinkToPick,
  addPendingLinks,
  addPendingLinksFromFiles,
  clipboardText,
  confirmWithUser,
  linksInCopy,
  linksInText,
  listingKind,
  pendingLinkPicked,
  showMainWindow,
} from "./api"
import type { ListingKind } from "./bindings/ListingKind"
import { ChaptersContext, type ChapterTarget, type ShownChapters } from "./chapters"
import { AddBar, type AddBarHandle } from "./components/AddBar"
import { AppTitle } from "./components/AppTitle"
import { Button } from "./components/Button"
import { Card } from "./components/Card"
import { IconButton } from "./components/IconButton"
import { PreviewCard } from "./components/PreviewCard"
import { Sidebar } from "./components/Sidebar"
import { StatusMessage } from "./components/StatusMessage"
import { classes } from "./components/styles"
import { ToastRegion } from "./components/UndoToast"
import { PixivWarningNotice } from "./components/PixivWarningNotice"
import { UpdateBanner } from "./components/UpdateBanner"
import { ViewTabs } from "./components/ViewTabs"
import { WindowControls } from "./components/WindowControls"
import { WhatsNewCard } from "./components/WhatsNewCard"
import { describeDownloadError, type ErrorDescription } from "./errors"
import { ASK_BEFORE_ADDING_OVER, checkingTime, type LinkFilesRead, readLinkFiles } from "./linkFiles"
import {
  AppActionsContext,
  mainNavItems,
  settingsNavItem,
  type SettingsPlace,
  type HistoryTarget,
  type SettingsTarget,
  type ViewId,
} from "./navigation"
import { shownPendingLinks } from "./pendingLinks"
import {
  useAddingOptions,
  useAppearanceOptions,
  useAppInfo,
  usePendingLinks,
  useQueue,
  useSaveAppearanceOptions,
  useToolsNeedSetup,
  useWatchedSources,
} from "./queries"
import { useAppShortcuts, useWindowPaste } from "./shortcuts"
import { useSidebarFold } from "./sidebarFold"
import { strings } from "./strings"
import { keptPicker } from "./keptPickers"
import { DetailsDockContext } from "./tileRows"
import { OfferToastContext, useToasts } from "./undo"
import { type Drop, useLinkDrop, usePreview } from "./usePreview"
import { HistoryView } from "./views/HistoryView"
import { PendingView } from "./views/PendingView"
import { PresetsView } from "./views/PresetsView"
import { QueueView } from "./views/QueueView"
import { RulesView } from "./views/RulesView"
import { SettingsView } from "./views/SettingsView"
import { SetupView } from "./views/SetupView"
import { WatchedView } from "./views/WatchedView"

/** How long Paste Link's note stays beside it. */
const PASTE_NOTE_MS = 6000

export function App() {
  const [view, setShownView] = useState<ViewId>("queue")
  // What the history shows when a watched source's Show Downloads in History opens it, until another view is opened.
  const [historyTarget, setHistoryTarget] = useState<HistoryTarget | null>(null)
  // The open view's chapters, which the sidebar lists under it (`useChapters`), and the one asked for last there.
  const [chapters, setChapters] = useState<ShownChapters | null>(null)
  const [chapterTarget, setChapterTarget] = useState<ChapterTarget | null>(null)
  const withdrawChapters = useCallback(
    (closed: ViewId) => setChapters((shown) => (shown?.view === closed ? null : shown)),
    [],
  )
  const chaptersLink = useMemo(
    () => ({ publish: setChapters, withdraw: withdrawChapters, target: chapterTarget }),
    [withdrawChapters, chapterTarget],
  )
  const setView = useCallback((to: ViewId) => {
    setHistoryTarget(null)
    setChapterTarget(null)
    setShownView(to)
  }, [])
  // The text of the add bar's field, shown once Type a Link opens it: the link shown last, or what was typed.
  const [link, setLink] = useState("")
  const [addError, setAddError] = useState<ErrorDescription | null>(null)
  // Why Paste Link added nothing, beside it for a few seconds: a new object each time, so saying it again starts the
  // wait again.
  const [pasteNote, setPasteNote] = useState<{ text: string } | null>(null)
  useEffect(() => {
    if (!pasteNote) return
    const timer = setTimeout(() => setPasteNote(null), PASTE_NOTE_MS)
    return () => clearTimeout(timer)
  }, [pasteNote])
  /** Why dropped files gave no links: a type Tsundoku doesn't read, or none in it. */
  const [fileProblems, setFileProblems] = useState<ErrorDescription[]>([])
  const { data: needsSetup } = useToolsNeedSetup()
  const preview = usePreview()
  const { show: showPreview, close: closePreview } = preview
  const { data: queue = [] } = useQueue()
  const { data: pending = [] } = usePendingLinks()
  const { data: watched = [] } = useWatchedSources()
  const newlyWatched = watched.reduce((count, source) => count + source.newItems.length, 0)
  const unfinished = queue.filter((item) => item.state !== "done").length

  const { data: appInfo } = useAppInfo()
  const startedInTray = appInfo?.startedInTray ?? false
  // Folded to a narrow strip, the sidebar keeps the add bar's buttons as icons and hides the chapters. Kept in
  // settings.json, which main.tsx reads before the first render, so it never shows open first; folded at once, and
  // still folded if saving fails (it's then only forgotten at the next start). Its width eases from one to the other.
  const { data: appearance } = useAppearanceOptions()
  const { save: saveAppearance } = useSaveAppearanceOptions()
  const sidebar = useSidebarFold(appearance?.sidebarCollapsed ?? false)
  const toggleSidebar = () => {
    sidebar.toggle()
    void saveAppearance({ sidebarCollapsed: !sidebar.collapsed })
  }
  const folded = sidebar.stripLayout
  // Where a picture's row shows in the Downloads, Pending and History views: under the view, across the content layer.
  const [detailsDock, setDetailsDock] = useState<HTMLDivElement | null>(null)

  useEffect(() => {
    // Started by Windows with the tray on, the window waits there until it's opened.
    if (startedInTray) return
    // After the first frame is painted, so the hidden window never shows blank.
    const frame = requestAnimationFrame(() => void showMainWindow())
    return () => cancelAnimationFrame(frame)
  }, [startedInTray])

  // The toast at the bottom of the view (Undo, Open in Pending), kept here so it stays when another view opens.
  const toasts = useToasts()
  const offerToast = toasts.show
  const { data: addingOptions } = useAddingOptions()
  // The open view, for what a link turns out to be once the core answers, after the view may have changed.
  const viewNow = useRef(view)
  useEffect(() => {
    viewNow.current = view
  }, [view])

  // The pending link whose items the preview card shows, opened from its row in Pending's Needs Attention: it leaves
  // Pending once items picked from it join the queue or it's watched. Any other link shown forgets it.
  const pickingFor = useRef<number | null>(null)
  // The same, for the picker, which keeps its ticks for the link while it waits in Pending.
  const [pickingId, setPickingId] = useState<number | null>(null)
  // Counts the links shown, so one found to be a channel after a newer link was shown doesn't replace that one.
  const shownCount = useRef(0)
  const { showListing } = preview
  // One picker shows at a time: opening another sends this one back to Needs Attention, its listing kept as loaded,
  // with its ticks (`keptPickers`), so opening it again shows it at once, without reading it again.
  const pickFromPending = useCallback(
    (id: number, shown: string) => {
      shownCount.current += 1
      pickingFor.current = id
      setPickingId(id)
      setLink(shown)
      const kept = keptPicker(id)
      if (kept?.listing) showListing(kept.listing, [...kept.skipped])
      else void showPreview(shown)
    },
    [showPreview, showListing],
  )
  // Closing the card leaves its link waiting in Needs Attention, with what was kept for it.
  const closeCard = useCallback(() => {
    pickingFor.current = null
    setPickingId(null)
    closePreview()
  }, [closePreview])
  /**
   * A channel, playlist or account added (its address says so, or the preview card read it as one): its picker lives
   * in Pending only, so it waits in Needs Attention. With Pending open, its picker opens at once, or, when the card
   * shows it already (`shown`), the card picks for it; on another view, a toast offers to open it in Pending, and the
   * view stays as it was. So does a link shown before a newer one (`current` false).
   */
  const addToPick = useCallback(
    async (added: string, kind: ListingKind | null, shown: boolean, current = true) => {
      try {
        const id = await addLinkToPick(added)
        if (current && viewNow.current === "pending") {
          if (!shown) {
            pickFromPending(id, added)
            return
          }
          pickingFor.current = id
          setPickingId(id)
          return
        }
        if (current) {
          setLink("")
          closePreview()
        }
        offerToast({
          message: strings.addBar.toPick[kind ?? "other"],
          action: {
            label: strings.addBar.openInPending,
            run: () => {
              setView("pending")
              pickFromPending(id, added)
            },
          },
        })
      } catch (error) {
        setAddError(describeDownloadError(error))
      }
    },
    [pickFromPending, closePreview, offerToast, setView],
  )
  // A copied or dropped link, or Download Again in the history, is fetched right away, and kept as the add bar's
  // field's text, to edit there; one whose address says it's a channel, playlist or account goes to Pending instead.
  const showLink = useCallback(
    (shown: string) => {
      shownCount.current += 1
      const request = shownCount.current
      pickingFor.current = null
      setPickingId(null)
      setLink(shown)
      void listingKind(shown)
        .catch(() => null)
        .then((kind) => {
          const current = request === shownCount.current
          if (kind) void addToPick(shown, kind, false, current)
          else if (current) void showPreview(shown)
        })
    },
    [showPreview, addToPick],
  )
  // A link shown that reading found to list several items (another site's playlist) goes to Pending as well.
  const shownState = preview.state
  useEffect(() => {
    if (shownState.stage !== "ready" || shownState.preview.kind !== "listing" || pickingFor.current !== null) return
    const { listing } = shownState.preview
    void addToPick(listing.url, listing.kind, true)
  }, [shownState, addToPick])
  // A picker shows above Pending only; elsewhere it stays as it is, out of sight.
  const picking = pickingId !== null || (shownState.stage === "ready" && shownState.preview.kind === "listing")
  const previewHidden = picking && view !== "pending"
  const donePicking = useCallback(() => {
    const id = pickingFor.current
    pickingFor.current = null
    setPickingId(null)
    if (id !== null) pendingLinkPicked(id).catch((error: unknown) => setAddError(describeDownloadError(error)))
  }, [])
  // Several links wait in Pending, where they're read one at a time.
  const addToPending = useCallback(
    async (links: string[]) => {
      try {
        await addPendingLinks(links)
        setLink("")
        closePreview()
        setShownView("pending")
      } catch (error) {
        setAddError(describeDownloadError(error))
      }
    },
    [closePreview],
  )
  // Text typed in the add bar's field, or dropped, with one link in it is previewed, and so is text with none, for
  // the card to say why. Several links go to Pending.
  const addText = useCallback(
    async (text: string) => {
      setAddError(null)
      setPasteNote(null)
      const links = await linksInText(text)
      if (links.length < 2) showLink(links[0] ?? text.trim())
      else await addToPending(links)
    },
    [showLink, addToPending],
  )
  // What was copied, added without being seen first (Paste Link, Ctrl+V outside a text field): one link is
  // previewed and several go to Pending, as above, but only links written out count among several words, and a copy
  // without any is said beside Paste Link rather than previewed.
  const addCopied = useCallback(
    async (text: string | null) => {
      setAddError(null)
      const links = text ? await linksInCopy(text) : []
      if (links.length === 0) {
        setPasteNote({ text: strings.addBar.noLinkCopied })
        return
      }
      setPasteNote(null)
      if (links.length === 1) showLink(links[0])
      else await addToPending(links)
    },
    [showLink, addToPending],
  )
  const takeCopied = useCallback((text: string) => void addCopied(text), [addCopied])
  const pasteFromClipboard = useCallback(async () => addCopied(await clipboardText()), [addCopied])

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
        setShownView("pending")
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

  // Ctrl+V outside a text field adds what was copied, as Paste Link does; Ctrl+L opens the add bar's field to type a
  // link, and Ctrl+1 to Ctrl+7 the sidebar's views. Not during the first-run setup.
  const addBar = useRef<AddBarHandle>(null)
  const shortcutsOn = needsSetup === false
  useWindowPaste(takeCopied, shortcutsOn)
  const typeLink = useCallback(() => addBar.current?.typeLink(), [])
  useAppShortcuts({ typeLink, navigate: setView }, shortcutsOn)

  // Where Settings opens, when another view sends the user to a place in it. A new object each time, so going to the
  // same place again scrolls there again.
  const [settingsPlace, setSettingsPlace] = useState<SettingsTarget | null>(null)
  const navigate = useCallback(
    (to: ViewId, place?: SettingsPlace) => {
      setView(to)
      setSettingsPlace(place ? { place } : null)
    },
    [setView],
  )
  const showWatchedDownloads = useCallback((sourceId: number) => {
    setShownView("history")
    setHistoryTarget({ watchedSource: sourceId })
  }, [])
  // The What's New card opened by hand (Settings → About), to browse every version's changes, until it's closed.
  const [browsingWhatsNew, setBrowsingWhatsNew] = useState(false)
  const showWhatsNew = useCallback(() => setBrowsingWhatsNew(true), [])
  // The pending link whose picker shows above Pending, which Needs Attention leaves out meanwhile.
  const openPickerId = preview.state.stage === "idle" ? null : pickingId
  const actions = useMemo(
    () => ({
      navigate,
      showLink,
      pickFromPending,
      openPickerId,
      pasteFromClipboard,
      addFileLinks: (read: LinkFilesRead) => void addFileLinks(read),
      showWatchedDownloads,
      showWhatsNew,
    }),
    [
      navigate,
      showLink,
      pickFromPending,
      openPickerId,
      pasteFromClipboard,
      addFileLinks,
      showWatchedDownloads,
      showWhatsNew,
    ],
  )

  if (needsSetup) return <SetupView />

  return (
    <AppActionsContext value={actions}>
      <OfferToastContext value={offerToast}>
        <div className="grid h-full grid-cols-[auto_minmax(0,1fr)]">
          {/* The app's name with Collapse Sidebar, Paste Link and Type a Link, then the open view's chapters; collapsed,
              Expand Sidebar and the two buttons as icons. The view's column takes the whole height. Its width eases
              over 200 ms (sidebarFold.ts), the content inside keeping the width of the layout drawn. The top row is
              the title bar's left end: its empty space moves the window, as the tabs' row does. */}
          <div
            className={classes(
              "min-h-0 transition-[width] duration-200 ease-decelerate motion-reduce:transition-none",
              sidebar.collapsed ? "w-14" : "w-52",
              sidebar.moving && "overflow-x-clip",
            )}
            onTransitionEnd={sidebar.onTransitionEnd}
          >
            <div className={classes("flex h-full min-h-0 flex-col", folded ? "w-14" : "w-52")}>
              <div
                data-tauri-drag-region="deep"
                className={classes("flex h-14 shrink-0 items-center", folded ? "justify-center" : "pr-2")}
              >
                {!folded && (
                  <div className="min-w-0 flex-1">
                    <AppTitle />
                  </div>
                )}
                <IconButton
                  label={sidebar.collapsed ? strings.nav.expandSidebar : strings.nav.collapseSidebar}
                  icon={sidebar.collapsed ? PanelLeftOpen : PanelLeftClose}
                  aria-expanded={!sidebar.collapsed}
                  onClick={toggleSidebar}
                />
              </div>
              <div className="relative z-10 px-2 pb-3">
                <AddBar
                  ref={addBar}
                  collapsed={folded}
                  onPaste={() => void pasteFromClipboard()}
                  note={pasteNote?.text ?? null}
                  onTypeLink={() => setPasteNote(null)}
                  link={link}
                  onLinkChange={setLink}
                  onSubmit={() => void addText(link)}
                />
              </div>
              {!folded && (
                <Sidebar
                  className="flex-1"
                  chapters={chapters?.view === view ? chapters : null}
                  viewLabel={[...mainNavItems, settingsNavItem].find((item) => item.id === view)?.label ?? ""}
                  onSelectChapter={(id) => setChapterTarget({ view, id })}
                />
              )}
            </div>
          </div>
          <div className="flex min-h-0 flex-col">
            {/* The rest of the title bar: the window has no frame of its own (title_bar.rs), so its empty space moves the
                window and a double-click on it maximizes, while the tabs and buttons stay buttons. */}
            <div data-tauri-drag-region="deep" className="flex h-14 shrink-0 items-center gap-2">
              <ViewTabs
                current={view}
                counts={{
                  queue: unfinished,
                  pending: shownPendingLinks(pending).length,
                  watched: newlyWatched,
                }}
                // The open view clicked again goes back to its top.
                onSelect={(to) => (to === view ? setChapterTarget({ view, id: null }) : setView(to))}
              />
              <WindowControls />
            </div>
            {/* Hidden while it has nothing to show, so it takes no room. */}
            <div className="flex flex-col gap-3 pr-4 pb-3 empty:hidden">
              <PixivWarningNotice />
              <UpdateBanner />
              <WhatsNewCard browsing={browsingWhatsNew} onStopBrowsing={() => setBrowsingWhatsNew(false)} />
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
            </div>
            {/* A picker stays as it is while out of sight, so going back to Pending finds it as it was. It gives up
                room before the view does, when notices above it take some. */}
            {preview.state.stage !== "idle" && (
              <div hidden={previewHidden} className="flex min-h-0 flex-col pr-4 pb-3">
                <PreviewCard
                  state={preview.state}
                  pendingId={pickingId}
                  startFullPage={addingOptions?.pickInFullPage ?? true}
                  hidden={previewHidden}
                  onClose={closeCard}
                  onRetry={(retryLink, password) => void preview.show(retryLink, password)}
                  onUnlock={(password) => void preview.unlock(password)}
                  onAdded={() => {
                    donePicking()
                    preview.close()
                    setLink("")
                  }}
                  onSomeAdded={donePicking}
                  onWatched={() => {
                    donePicking()
                    preview.close()
                  }}
                />
              </div>
            )}
            <main
              className={classes(
                "flex flex-1 flex-col rounded-tl-lg border-t border-l border-line bg-surface",
                // Under a picker, the view keeps room of its own; otherwise notices may take it all.
                picking && !previewHidden ? "min-h-40" : "min-h-0",
              )}
            >
              <div className="relative min-h-0 flex-1">
                <ChaptersContext value={chaptersLink}>
                  <DetailsDockContext value={detailsDock}>
                    {/* Fading in as it opens, a new element for each view. */}
                    <div key={view} className="h-full motion-safe:animate-view-in">
                      <CurrentView view={view} settingsPlace={settingsPlace} historyTarget={historyTarget} />
                    </div>
                  </DetailsDockContext>
                </ChaptersContext>
                {/* Outside the views, so an offer stays when another view is opened; above the details panel. */}
                <ToastRegion offer={toasts.offer} onClose={toasts.close} />
              </div>
              <div ref={setDetailsDock} className="shrink-0" />
            </main>
          </div>
        </div>
      </OfferToastContext>
    </AppActionsContext>
  )
}

interface CurrentViewProps {
  view: ViewId
  settingsPlace: SettingsTarget | null
  historyTarget: HistoryTarget | null
}

function CurrentView({ view, settingsPlace, historyTarget }: CurrentViewProps) {
  switch (view) {
    case "queue":
      return <QueueView />
    case "pending":
      return <PendingView />
    case "watched":
      return <WatchedView />
    case "history":
      return <HistoryView target={historyTarget} />
    case "presets":
      return <PresetsView />
    case "rules":
      return <RulesView />
    case "settings":
      return <SettingsView place={settingsPlace} />
  }
}

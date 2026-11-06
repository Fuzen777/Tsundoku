import { invoke, isTauri } from "@tauri-apps/api/core"
import { listen, type UnlistenFn } from "@tauri-apps/api/event"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { ask, open } from "@tauri-apps/plugin-dialog"
import { openUrl } from "@tauri-apps/plugin-opener"
import { relaunch } from "@tauri-apps/plugin-process"
import { check } from "@tauri-apps/plugin-updater"
import type { AddedLinks } from "./bindings/AddedLinks"
import type { AddingSettings } from "./bindings/AddingSettings"
import type { AppInfo } from "./bindings/AppInfo"
import type { AppearanceSettings } from "./bindings/AppearanceSettings"
import type { BackgroundSettings } from "./bindings/BackgroundSettings"
import type { Destination } from "./bindings/Destination"
import type { DetectedTool } from "./bindings/DetectedTool"
import type { DownloadProgressEvent } from "./bindings/DownloadProgressEvent"
import type { DownloadSettings } from "./bindings/DownloadSettings"
import type { FirstRunChoices } from "./bindings/FirstRunChoices"
import type { FirstRunOptions } from "./bindings/FirstRunOptions"
import type { FolderRule } from "./bindings/FolderRule"
import type { HistoryFile } from "./bindings/HistoryFile"
import type { HistoryFilters } from "./bindings/HistoryFilters"
import type { HistoryQuery } from "./bindings/HistoryQuery"
import type { HistoryVideo } from "./bindings/HistoryVideo"
import type { PastDownload } from "./bindings/PastDownload"
import type { PendingLink } from "./bindings/PendingLink"
import type { Preset } from "./bindings/Preset"
import type { PresetsOverview } from "./bindings/PresetsOverview"
import type { QueueItem } from "./bindings/QueueItem"
import type { SupportedSites } from "./bindings/SupportedSites"
import type { ToolCaches } from "./bindings/ToolCaches"
import type { ToolDownload } from "./bindings/ToolDownload"
import type { ToolId } from "./bindings/ToolId"
import type { ToolProgress } from "./bindings/ToolProgress"
import type { ToolStatus } from "./bindings/ToolStatus"
import type { ToolUpdateCheck } from "./bindings/ToolUpdateCheck"
import type { ToolsOverview } from "./bindings/ToolsOverview"
import type { UpdateChannel } from "./bindings/UpdateChannel"
import type { VideoInfo } from "./bindings/VideoInfo"
import type { VideoPreview } from "./bindings/VideoPreview"
import { standIns } from "./standIns"

/*
  The only place the UI calls the Rust side. Types come from src/bindings, generated from the Rust core.
  Outside Tauri (the UI opened in a plain browser to check a layout), reads return the stand-ins from standIns.ts and
  actions fail, instead of everything failing.
*/

export const queryKeys = {
  appInfo: ["app-info"],
  toolsNeedSetup: ["tools-need-setup"],
  firstRunOptions: ["first-run-options"],
  toolsOverview: ["tools-overview"],
  detectedTools: ["detected-tools"],
  toolDownloads: ["tool-downloads"],
  presets: ["presets"],
  presetArguments: ["preset-arguments"],
  folderRules: ["folder-rules"],
  downloadDestination: ["download-destination"],
  queue: ["queue"],
  pending: ["pending"],
  history: ["history"],
  historyFilters: ["history-filters"],
  videoDownloads: ["video-downloads"],
  downloadFiles: ["download-files"],
  downloadOptions: ["download-options"],
  addingOptions: ["adding-options"],
  backgroundOptions: ["background-options"],
  appearanceOptions: ["appearance-options"],
  defaultDownloadFolder: ["default-download-folder"],
  supportedSites: ["supported-sites"],
  appUpdate: ["app-update"],
} as const

/** Sent by the core while a tool is installed or updated. */
const TOOL_PROGRESS_EVENT = "tool-progress"
/** Sent by the core when a tool changed on its own, such as yt-dlp's daily update. */
const TOOLS_CHANGED_EVENT = "tools-changed"
/** Sent by the core with every queue item when the queue changes. */
const QUEUE_CHANGED_EVENT = "queue-changed"
/** Sent by the core while a download runs. */
const DOWNLOAD_PROGRESS_EVENT = "download-progress"
/** Sent by the core with every pending link when they change. */
const PENDING_CHANGED_EVENT = "pending-changed"
/** Sent by the app with the options when how links get added changes, such as from the tray's Watch Clipboard. */
const ADDING_OPTIONS_CHANGED_EVENT = "adding-options-changed"

function outsideTauri(): Promise<never> {
  return Promise.reject(new Error("This action needs the Tsundoku app, not a browser."))
}

/** In a browser, whether the address has `?name` (`?setup`, `&portable`), to see what the stand-ins don't show. */
function addressHas(name: string): boolean {
  return new URLSearchParams(window.location.search).has(name)
}

/** In a browser, adding `?portable` to the address shows a portable copy. */
export function getAppInfo(): Promise<AppInfo> {
  if (!isTauri()) return Promise.resolve({ ...standIns.appInfo, portable: addressHas("portable") })
  return invoke<AppInfo>("app_info")
}

/** Opens the data folder in File Explorer. Rejects with Windows' reason as a string. */
export async function openDataFolder(): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("open_data_folder")
}

/** The window starts hidden (tauri.conf.json); the UI shows it once its first frame is painted. */
export async function showMainWindow(): Promise<void> {
  if (!isTauri()) return
  await getCurrentWindow().show()
}

/** Whether the first-run setup is needed. In a browser, adding `?setup` to the address shows it. */
export function getToolsNeedSetup(): Promise<boolean> {
  if (!isTauri()) return Promise.resolve(addressHas("setup"))
  return invoke<boolean>("tools_need_setup")
}

/** The first-run setup's choices (where the tools go, their caches, the download folder) and whether they're still to
 * be made. In a browser, they are, with an installed copy's folders; `?setup&portable` shows a portable copy's. */
export function getFirstRunOptions(): Promise<FirstRunOptions> {
  if (!isTauri()) return Promise.resolve(standIns.firstRunOptions(addressHas("portable")))
  return invoke<FirstRunOptions>("first_run_options")
}

/** Saves the first-run choices, before the tools are set up. Rejects with a DownloadError. */
export async function saveFirstRunChoices(choices: FirstRunChoices): Promise<void> {
  if (!isTauri()) return
  await invoke("save_first_run_choices", { choices })
}

/** Every tool's status. The core runs each tool to read its version, so this takes a moment. */
export function getToolsOverview(): Promise<ToolsOverview> {
  if (!isTauri()) return Promise.resolve(standIns.toolsOverview)
  return invoke<ToolsOverview>("tools_overview")
}

/** Copies of the tools already on this PC, for the first-run setup. */
export function detectTools(): Promise<DetectedTool[]> {
  if (!isTauri()) return Promise.resolve(standIns.detectedTools)
  return invoke<DetectedTool[]>("detect_tools")
}

export function getToolDownloads(): Promise<ToolDownload[]> {
  if (!isTauri()) return Promise.resolve(standIns.toolDownloads)
  return invoke<ToolDownload[]>("tool_downloads")
}

/** Downloads the tool's official build as Tsundoku's own copy. Progress arrives through `onToolProgress`. */
export function installTool(id: ToolId): Promise<ToolStatus> {
  if (!isTauri()) return outsideTauri()
  return invoke<ToolStatus>("install_tool", { id })
}

/** Sets up a copy already on this PC. */
export function chooseToolCopy(id: ToolId, path: string): Promise<ToolStatus> {
  if (!isTauri()) return outsideTauri()
  return invoke<ToolStatus>("use_tool_copy", { id, path })
}

/** Stops the tool's install or update; the action then fails as cancelled. */
export async function cancelToolOperation(id: ToolId): Promise<void> {
  if (!isTauri()) return
  await invoke("cancel_tool_operation", { id })
}

export function updateYtDlp(): Promise<ToolStatus> {
  if (!isTauri()) return outsideTauri()
  return invoke<ToolStatus>("update_yt_dlp")
}

export function checkToolUpdate(id: ToolId): Promise<ToolUpdateCheck> {
  if (!isTauri()) return outsideTauri()
  return invoke<ToolUpdateCheck>("check_tool_update", { id })
}

export async function setYtDlpChannel(channel: UpdateChannel): Promise<void> {
  if (!isTauri()) return
  await invoke("set_yt_dlp_channel", { channel })
}

/** Where the tools keep their caches, from their next run on. Rejects with a DownloadError. */
export async function setToolCaches(caches: ToolCaches): Promise<void> {
  if (!isTauri()) return
  await invoke("set_tool_caches", { caches })
}

/** Asks for a program file. Resolves to its path, or null if the user cancelled. */
export async function pickProgram(title: string, filterName: string): Promise<string | null> {
  if (!isTauri()) return null
  return open({ title, multiple: false, directory: false, filters: [{ name: filterName, extensions: ["exe"] }] })
}

/**
 * What a link is, and whether it was downloaded before. Rejects with a PreviewError. Fetching another preview cancels
 * this one, which then rejects as cancelled.
 */
export function fetchPreview(link: string): Promise<VideoPreview> {
  if (!isTauri()) return standIns.fetchPreview(link)
  return invoke<VideoPreview>("fetch_preview", { link })
}

export async function cancelPreview(): Promise<void> {
  if (!isTauri()) return
  await invoke("cancel_preview")
}

/** Opens a downloaded file in its default app. Rejects with a HistoryError. */
export async function openDownload(downloadId: number): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("open_download", { downloadId })
}

/** Opens File Explorer on a download's folder, with the file selected. Rejects with a HistoryError. */
export async function showDownloadInFolder(downloadId: number): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("show_download_in_folder", { downloadId })
}

export function onToolProgress(handler: (progress: ToolProgress) => void): Promise<UnlistenFn> {
  if (!isTauri()) return Promise.resolve(() => {})
  return listen<ToolProgress>(TOOL_PROGRESS_EVENT, (event) => handler(event.payload))
}

export function onToolsChanged(handler: () => void): Promise<UnlistenFn> {
  if (!isTauri()) return Promise.resolve(() => {})
  return listen(TOOLS_CHANGED_EVENT, () => handler())
}

/** The presets, the default and the download folder. */
export function getPresetsOverview(): Promise<PresetsOverview> {
  if (!isTauri()) return Promise.resolve(standIns.presetsOverview)
  return invoke<PresetsOverview>("presets_overview")
}

/** Saves a preset, replacing the one with its ID. Rejects with a PresetError. */
export function savePreset(preset: Preset): Promise<PresetsOverview> {
  if (!isTauri()) return outsideTauri()
  return invoke<PresetsOverview>("save_preset", { preset })
}

/** Adds a copy of a preset after it, under `name`, and resolves to the copy. */
export function duplicatePreset(id: string, name: string): Promise<Preset> {
  if (!isTauri()) return outsideTauri()
  return invoke<Preset>("duplicate_preset", { id, name })
}

export function deletePreset(id: string): Promise<PresetsOverview> {
  if (!isTauri()) return outsideTauri()
  return invoke<PresetsOverview>("delete_preset", { id })
}

/** Puts a built-in preset back as it ships. */
export function resetPreset(id: string): Promise<PresetsOverview> {
  if (!isTauri()) return outsideTauri()
  return invoke<PresetsOverview>("reset_preset", { id })
}

export function setDefaultPreset(id: string): Promise<PresetsOverview> {
  if (!isTauri()) return outsideTauri()
  return invoke<PresetsOverview>("set_default_preset", { id })
}

/** The yt-dlp arguments a preset gives. Rejects with a PresetError when its extra arguments can't be used. */
export function getPresetArguments(preset: Preset): Promise<string[]> {
  if (!isTauri()) return Promise.resolve(standIns.presetArguments)
  return invoke<string[]>("preset_arguments", { preset })
}

export function getQueueItems(): Promise<QueueItem[]> {
  if (!isTauri()) return Promise.resolve(standIns.queueItems)
  return invoke<QueueItem[]>("queue_items")
}

/** Adds a video to the download queue. Rejects with a DownloadError. */
export function addToQueue(video: VideoInfo, presetId: string, overwrite: boolean): Promise<QueueItem> {
  if (!isTauri()) return outsideTauri()
  return invoke<QueueItem>("add_to_queue", { video, presetId, overwrite })
}

/** Adds a post of images, whose files all go into the site's folder in the posts folder. */
export function addPostToQueue(video: VideoInfo): Promise<QueueItem> {
  if (!isTauri()) return outsideTauri()
  return invoke<QueueItem>("add_post_to_queue", { video })
}

/** The folder rules, in the order they're tried. */
export function getFolderRules(): Promise<FolderRule[]> {
  if (!isTauri()) return Promise.resolve(standIns.folderRules)
  return invoke<FolderRule[]>("folder_rules")
}

/** Replaces every folder rule, and resolves to them as saved. Rejects with a RuleError for a rule without a folder
 * or with an empty condition. */
export function saveFolderRules(rules: FolderRule[]): Promise<FolderRule[]> {
  if (!isTauri()) return outsideTauri()
  return invoke<FolderRule[]>("save_folder_rules", { rules })
}

/** Where a download would go and what decided it: with a preset for a video, none for a post. */
export function getDownloadDestination(video: VideoInfo, presetId: string | null): Promise<Destination> {
  if (!isTauri()) return Promise.resolve(standIns.destination)
  return invoke<Destination>("download_destination", { video, presetId })
}

/** Where downloads go when no folder is chosen: a Tsundoku folder in Downloads. */
export function getDefaultDownloadFolder(): Promise<string> {
  if (!isTauri()) return Promise.resolve(standIns.defaultDownloadFolder)
  return invoke<string>("default_download_folder")
}

/** Stops a running download, keeping its partial files, or holds a waiting one. */
export async function stopDownload(id: number): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("stop_download", { id })
}

export async function resumeDownload(id: number): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("resume_download", { id })
}

export async function retryDownload(id: number): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("retry_download", { id })
}

/** Removes a download that isn't done, with its partial files. */
export async function cancelDownload(id: number): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("cancel_download", { id })
}

export async function clearFinishedDownloads(): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("clear_finished_downloads")
}

/** Opens a download's log in the default text editor. Rejects with a DownloadError. */
export async function openDownloadLog(id: number): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("open_download_log", { id })
}

export function onQueueChanged(handler: (items: QueueItem[]) => void): Promise<UnlistenFn> {
  if (!isTauri()) return Promise.resolve(() => {})
  return listen<QueueItem[]>(QUEUE_CHANGED_EVENT, (event) => handler(event.payload))
}

/** Links waiting to be started, oldest first. */
export function getPendingLinks(): Promise<PendingLink[]> {
  if (!isTauri()) return Promise.resolve(standIns.pendingLinks)
  return invoke<PendingLink[]>("pending_links")
}

/** The links in pasted or dropped text, each once, so one link can be told from several. */
export function linksInText(text: string): Promise<string[]> {
  if (!isTauri()) return Promise.resolve(standIns.linksInText(text))
  return invoke<string[]>("links_in_text", { text })
}

/** Adds links to the pending ones, which are then read one at a time. Rejects with a DownloadError. */
export function addPendingLinks(links: string[]): Promise<AddedLinks> {
  if (!isTauri()) return outsideTauri()
  return invoke<AddedLinks>("add_pending_links", { links })
}

export async function removePendingLinks(ids: number[]): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("remove_pending_links", { ids })
}

/** Reads a link that couldn't be read again, after a fix such as signing in. */
export async function readPendingLinkAgain(id: number): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("read_pending_link_again", { id })
}

/** Picks one link's preset, or null for the one picked for them all. */
export async function setPendingPreset(id: number, presetId: string | null): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("set_pending_preset", { id, presetId })
}

/**
 * Starts links: those read join the queue with their own preset or `presetId`, the others once they're read. A video
 * downloaded before with the same preset replaces its file, so ask first. Rejects with a DownloadError.
 */
export async function startPendingLinks(ids: number[], presetId: string): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("start_pending_links", { ids, presetId })
}

/** How links get added besides pasting them: the clipboard watcher and Download Right Away. */
export function getAddingOptions(): Promise<AddingSettings> {
  if (!isTauri()) return Promise.resolve(standIns.addingOptions)
  return invoke<AddingSettings>("adding_options")
}

/** Saves how links get added, and resolves to the options as saved. Rejects with a DownloadError, such as when
 * another app has the clipboard shortcut picked. */
export function saveAddingOptions(options: AddingSettings): Promise<AddingSettings> {
  if (!isTauri()) return outsideTauri()
  return invoke<AddingSettings>("save_adding_options", { options })
}

export function onAddingOptionsChanged(handler: (options: AddingSettings) => void): Promise<UnlistenFn> {
  if (!isTauri()) return Promise.resolve(() => {})
  return listen<AddingSettings>(ADDING_OPTIONS_CHANGED_EVENT, (event) => handler(event.payload))
}

/** Puts text on the clipboard, such as the browser bookmark's address. Tsundoku's own copies never reach the
 * clipboard watcher. */
export function copyText(text: string): Promise<void> {
  return navigator.clipboard.writeText(text)
}

/** Whether closing the window keeps Tsundoku in the tray, and whether it starts with Windows. */
export function getBackgroundOptions(): Promise<BackgroundSettings> {
  if (!isTauri()) return Promise.resolve(standIns.backgroundOptions)
  return invoke<BackgroundSettings>("background_options")
}

/** Saves Settings → Running in the Background, and resolves to the options as saved. Rejects with a DownloadError. */
export function saveBackgroundOptions(options: BackgroundSettings): Promise<BackgroundSettings> {
  if (!isTauri()) return outsideTauri()
  return invoke<BackgroundSettings>("save_background_options", { options })
}

/** Whether Tsundoku is light, dark, or follows Windows. main.tsx reads it before the first render. */
export function getAppearanceOptions(): Promise<AppearanceSettings> {
  if (!isTauri()) return Promise.resolve(standIns.appearanceOptions)
  return invoke<AppearanceSettings>("appearance_options")
}

/** Saves Settings → Appearance, which also gives the window the theme, and resolves to the options as saved. Rejects
 * with a DownloadError. */
export function saveAppearanceOptions(options: AppearanceSettings): Promise<AppearanceSettings> {
  if (!isTauri()) return outsideTauri()
  return invoke<AppearanceSettings>("save_appearance_options", { options })
}

export function onPendingChanged(handler: (links: PendingLink[]) => void): Promise<UnlistenFn> {
  if (!isTauri()) return Promise.resolve(() => {})
  return listen<PendingLink[]>(PENDING_CHANGED_EVENT, (event) => handler(event.payload))
}

export function onDownloadProgress(handler: (event: DownloadProgressEvent) => void): Promise<UnlistenFn> {
  if (!isTauri()) return Promise.resolve(() => {})
  return listen<DownloadProgressEvent>(DOWNLOAD_PROGRESS_EVENT, (event) => handler(event.payload))
}

/** Asks for a folder. Resolves to its path, or null if the user cancelled. */
export async function pickFolder(title: string): Promise<string | null> {
  if (!isTauri()) return null
  return open({ title, directory: true, multiple: false })
}

interface Confirmation {
  title: string
  message: string
  confirmLabel: string
  cancelLabel: string
}

/** Asks the user to confirm something with Windows' own dialog. In a browser, the page's. */
export async function confirmWithUser({ title, message, confirmLabel, cancelLabel }: Confirmation): Promise<boolean> {
  if (!isTauri()) return window.confirm(`${title}

${message}`)
  return ask(message, { title, kind: "warning", okLabel: confirmLabel, cancelLabel })
}

/** What closing the window does: closes Tsundoku, hides the window with Tsundoku running on in the tray, or nothing,
 * when the user would rather keep it open. */
export type CloseChoice = "close" | "hide" | "stay"

/** Lets the UI decide what closing the window does, such as asking first while downloads run. */
export async function onCloseRequested(choose: () => Promise<CloseChoice>): Promise<UnlistenFn> {
  if (!isTauri()) return () => {}
  const window = getCurrentWindow()
  return window.onCloseRequested(async (event) => {
    const choice = await choose()
    if (choice === "close") return
    event.preventDefault()
    if (choice === "hide") await window.hide()
  })
}

/** The history matching a search, filters and a sort, each video with its latest download. */
export function searchHistory(query: HistoryQuery): Promise<HistoryVideo[]> {
  if (!isTauri()) return Promise.resolve(standIns.searchHistory(query))
  return invoke<HistoryVideo[]>("search_history", { query })
}

/** The sites and presets in the history, for its filters. */
export function getHistoryFilters(): Promise<HistoryFilters> {
  if (!isTauri()) return Promise.resolve(standIns.historyFilters)
  return invoke<HistoryFilters>("history_filters")
}

/** Every download of a video in the history, newest first. */
export function getVideoDownloads(videoId: number): Promise<PastDownload[]> {
  if (!isTauri()) return Promise.resolve(standIns.videoDownloads)
  return invoke<PastDownload[]>("video_downloads", { videoId })
}

/** Every file of a download, in order, with whether each is still there. */
export function getDownloadFiles(downloadId: number): Promise<HistoryFile[]> {
  if (!isTauri()) return Promise.resolve(standIns.downloadFiles)
  return invoke<HistoryFile[]>("download_files", { downloadId })
}

/** Opens one file of a download in its default app. Rejects with a HistoryError. */
export async function openDownloadFile(downloadId: number, position: number): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("open_download_file", { downloadId, position })
}

/** Opens File Explorer on one file of a download, with it selected. Rejects with a HistoryError. */
export async function showDownloadFileInFolder(downloadId: number, position: number): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("show_download_file_in_folder", { downloadId, position })
}

/** Removes a video from the history and yt-dlp's archive, so it can be downloaded again. Its files stay. */
export async function forgetVideo(videoId: number): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("forget_video", { videoId })
}

/** Opens a web page, such as a video's own page, in the default browser. */
export async function openWebPage(url: string): Promise<void> {
  if (!isTauri()) {
    window.open(url, "_blank", "noopener")
    return
  }
  await openUrl(url)
}

/** Every site the tools can read, with the browser profiles signed in to the sites that need an account. Takes a
 * few seconds: it runs both tools and reads the browsers' cookie names. */
export function getSupportedSites(): Promise<SupportedSites> {
  if (!isTauri()) return Promise.resolve(standIns.supportedSites)
  return invoke<SupportedSites>("supported_sites")
}

/** How downloads run: the download folder, how many at once, the speed limit, the cookies browser and the pause
 * between a post's files. */
export function getDownloadOptions(): Promise<DownloadSettings> {
  if (!isTauri()) return Promise.resolve(standIns.downloadOptions)
  return invoke<DownloadSettings>("download_options")
}

/** Saves how downloads run, and resolves to the options as saved. Rejects with a DownloadError for a speed limit
 * yt-dlp wouldn't understand. */
export function saveDownloadOptions(options: DownloadSettings): Promise<DownloadSettings> {
  if (!isTauri()) return outsideTauri()
  return invoke<DownloadSettings>("save_download_options", { options })
}

/** A newer release of Tsundoku, found on GitHub. */
export interface AppUpdate {
  version: string
  /** Downloads the update, reporting bytes received, then installs it and restarts the app. */
  install: (onProgress: (received: number, total: number | null) => void) => Promise<void>
}

/**
 * Asks GitHub for a newer release, through the updater's latest.json. Resolves to null when there's none, and always
 * in development or a browser. Rejects when GitHub can't be asked, as while the repository is private.
 */
export async function checkForAppUpdate(): Promise<AppUpdate | null> {
  if (!isTauri() || import.meta.env.DEV) return null
  const update = await check()
  if (!update) return null
  return {
    version: update.version,
    install: async (onProgress) => {
      let received = 0
      let total: number | null = null
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? null
        } else if (event.event === "Progress") {
          received += event.data.chunkLength
          onProgress(received, total)
        }
      })
      // On Windows the installer closes the app itself; elsewhere it has to restart.
      await relaunch()
    },
  }
}

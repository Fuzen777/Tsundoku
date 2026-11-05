import { invoke, isTauri } from "@tauri-apps/api/core"
import { listen, type UnlistenFn } from "@tauri-apps/api/event"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { ask, open } from "@tauri-apps/plugin-dialog"
import { openUrl } from "@tauri-apps/plugin-opener"
import type { AppInfo } from "./bindings/AppInfo"
import type { DetectedTool } from "./bindings/DetectedTool"
import type { DownloadProgressEvent } from "./bindings/DownloadProgressEvent"
import type { DownloadSettings } from "./bindings/DownloadSettings"
import type { HistoryFilters } from "./bindings/HistoryFilters"
import type { HistoryQuery } from "./bindings/HistoryQuery"
import type { HistoryVideo } from "./bindings/HistoryVideo"
import type { PastDownload } from "./bindings/PastDownload"
import type { Preset } from "./bindings/Preset"
import type { PresetsOverview } from "./bindings/PresetsOverview"
import type { QueueItem } from "./bindings/QueueItem"
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
  toolsOverview: ["tools-overview"],
  detectedTools: ["detected-tools"],
  toolDownloads: ["tool-downloads"],
  presets: ["presets"],
  presetArguments: ["preset-arguments"],
  queue: ["queue"],
  history: ["history"],
  historyFilters: ["history-filters"],
  videoDownloads: ["video-downloads"],
  downloadOptions: ["download-options"],
} as const

/** Sent by the core while a tool is installed or updated. */
const TOOL_PROGRESS_EVENT = "tool-progress"
/** Sent by the core when a tool changed on its own, such as yt-dlp's daily update. */
const TOOLS_CHANGED_EVENT = "tools-changed"
/** Sent by the core with every queue item when the queue changes. */
const QUEUE_CHANGED_EVENT = "queue-changed"
/** Sent by the core while a download runs. */
const DOWNLOAD_PROGRESS_EVENT = "download-progress"

function outsideTauri(): Promise<never> {
  return Promise.reject(new Error("This action needs the Tsundoku app, not a browser."))
}

export function getAppInfo(): Promise<AppInfo> {
  if (!isTauri()) return Promise.resolve(standIns.appInfo)
  return invoke<AppInfo>("app_info")
}

/** The window starts hidden (tauri.conf.json); the UI shows it once its first frame is painted. */
export async function showMainWindow(): Promise<void> {
  if (!isTauri()) return
  await getCurrentWindow().show()
}

/** Whether the first-run setup is needed. In a browser, adding `?setup` to the address shows it. */
export function getToolsNeedSetup(): Promise<boolean> {
  if (!isTauri()) return Promise.resolve(new URLSearchParams(window.location.search).has("setup"))
  return invoke<boolean>("tools_need_setup")
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

/** The presets, the default and the default folders. */
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

/** Lets the UI keep the window open, such as while downloads run: `shouldClose` resolving to false cancels closing. */
export async function onCloseRequested(shouldClose: () => Promise<boolean>): Promise<UnlistenFn> {
  if (!isTauri()) return () => {}
  return getCurrentWindow().onCloseRequested(async (event) => {
    if (!(await shouldClose())) event.preventDefault()
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

/** How downloads run: how many at once, the speed limit and the cookies browser. */
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

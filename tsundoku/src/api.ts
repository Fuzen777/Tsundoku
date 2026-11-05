import { invoke, isTauri } from "@tauri-apps/api/core"
import { listen, type UnlistenFn } from "@tauri-apps/api/event"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { open } from "@tauri-apps/plugin-dialog"
import type { AppInfo } from "./bindings/AppInfo"
import type { DetectedTool } from "./bindings/DetectedTool"
import type { ToolDownload } from "./bindings/ToolDownload"
import type { ToolId } from "./bindings/ToolId"
import type { ToolProgress } from "./bindings/ToolProgress"
import type { ToolStatus } from "./bindings/ToolStatus"
import type { ToolUpdateCheck } from "./bindings/ToolUpdateCheck"
import type { ToolsOverview } from "./bindings/ToolsOverview"
import type { UpdateChannel } from "./bindings/UpdateChannel"
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
} as const

/** Sent by the core while a tool is installed or updated. */
const TOOL_PROGRESS_EVENT = "tool-progress"
/** Sent by the core when a tool changed on its own, such as yt-dlp's daily update. */
const TOOLS_CHANGED_EVENT = "tools-changed"

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

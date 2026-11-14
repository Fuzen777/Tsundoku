import { convertFileSrc, invoke, isTauri } from "@tauri-apps/api/core"
import { listen, type UnlistenFn } from "@tauri-apps/api/event"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { ask, open, save } from "@tauri-apps/plugin-dialog"
import { openUrl } from "@tauri-apps/plugin-opener"
import { relaunch } from "@tauri-apps/plugin-process"
import { check } from "@tauri-apps/plugin-updater"
import type { AddedLinks } from "./bindings/AddedLinks"
import type { AddingSettings } from "./bindings/AddingSettings"
import type { AppInfo } from "./bindings/AppInfo"
import type { AppearanceSettings } from "./bindings/AppearanceSettings"
import type { BackgroundSettings } from "./bindings/BackgroundSettings"
import type { ChangelogSection } from "./bindings/ChangelogSection"
import type { ClickNLoadStatus } from "./bindings/ClickNLoadStatus"
import type { Destination } from "./bindings/Destination"
import type { DetectedTool } from "./bindings/DetectedTool"
import type { DownloadLog } from "./bindings/DownloadLog"
import type { DownloadProgressEvent } from "./bindings/DownloadProgressEvent"
import type { DownloadSettings } from "./bindings/DownloadSettings"
import type { FirstRunChoices } from "./bindings/FirstRunChoices"
import type { FirstRunOptions } from "./bindings/FirstRunOptions"
import type { FolderRule } from "./bindings/FolderRule"
import type { HistoryFile } from "./bindings/HistoryFile"
import type { HistoryFilters } from "./bindings/HistoryFilters"
import type { HistoryQuery } from "./bindings/HistoryQuery"
import type { HistoryPage } from "./bindings/HistoryPage"
import type { LinkPreview } from "./bindings/LinkPreview"
import type { ListArrangements } from "./bindings/ListArrangements"
import type { PickerLayouts } from "./bindings/PickerLayouts"
import type { ListingKind } from "./bindings/ListingKind"
import type { MovedFiles } from "./bindings/MovedFiles"
import type { PastDownload } from "./bindings/PastDownload"
import type { PendingLink } from "./bindings/PendingLink"
import type { ItemKey } from "./bindings/ItemKey"
import type { NewWatchedSource } from "./bindings/NewWatchedSource"
import type { WatchedSource } from "./bindings/WatchedSource"
import type { Preset } from "./bindings/Preset"
import type { PresetsOverview } from "./bindings/PresetsOverview"
import type { QueueItem } from "./bindings/QueueItem"
import type { QueuePick } from "./bindings/QueuePick"
import type { RightAwayRule } from "./bindings/RightAwayRule"
import type { RulePreset } from "./bindings/RulePreset"
import type { ShownError } from "./bindings/ShownError"
import type { SupportedSites } from "./bindings/SupportedSites"
import type { ToolCaches } from "./bindings/ToolCaches"
import type { ToolDownload } from "./bindings/ToolDownload"
import type { ToolId } from "./bindings/ToolId"
import type { ToolProgress } from "./bindings/ToolProgress"
import type { ToolStatus } from "./bindings/ToolStatus"
import type { ToolUpdateCheck } from "./bindings/ToolUpdateCheck"
import type { ToolUpdateOutcome } from "./bindings/ToolUpdateOutcome"
import type { ToolsOverview } from "./bindings/ToolsOverview"
import type { UnpackingRule } from "./bindings/UnpackingRule"
import type { UpdateChannel } from "./bindings/UpdateChannel"
import type { VideoInfo } from "./bindings/VideoInfo"
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
  downloadLog: ["download-log"],
  rulePreset: ["rule-preset"],
  ruleRightAway: ["rule-right-away"],
  ruleUnpacking: ["rule-unpacking"],
  queue: ["queue"],
  pending: ["pending"],
  watched: ["watched"],
  listArrangements: ["list-arrangements"],
  pickerLayouts: ["picker-layouts"],
  history: ["history"],
  historyFilters: ["history-filters"],
  videoDownloads: ["video-downloads"],
  downloadFiles: ["download-files"],
  leftOutFiles: ["left-out-files"],
  leftOutSharedFiles: ["left-out-shared-files"],
  downloadOptions: ["download-options"],
  addingOptions: ["adding-options"],
  clickNLoad: ["click-n-load"],
  backgroundOptions: ["background-options"],
  appearanceOptions: ["appearance-options"],
  defaultDownloadFolder: ["default-download-folder"],
  supportedSites: ["supported-sites"],
  appUpdate: ["app-update"],
  whatsNew: ["whats-new"],
} as const

/** Sent by the core while a tool is installed or updated. */
const TOOL_PROGRESS_EVENT = "tool-progress"
/** Sent by the core when a tool changed on its own, such as yt-dlp's or gallery-dl's daily update. */
const TOOLS_CHANGED_EVENT = "tools-changed"
/** Sent by the core with every queue item when the queue changes. */
const QUEUE_CHANGED_EVENT = "queue-changed"
/** Sent by the core while a download runs. */
const DOWNLOAD_PROGRESS_EVENT = "download-progress"
/** Sent by the core with every pending link when they change. */
const PENDING_CHANGED_EVENT = "pending-changed"
const WATCHED_CHANGED_EVENT = "watched-changed"
/** Sent by the app with the options when how links get added changes, such as from the tray's Watch Clipboard. */
const ADDING_OPTIONS_CHANGED_EVENT = "adding-options-changed"
/** Sent by the app when the Click'n'Load receiver starts or stops listening, or can't. */
const CLICK_N_LOAD_CHANGED_EVENT = "click-n-load-changed"

function outsideTauri(): Promise<never> {
  return Promise.reject(new Error("This action needs the Tsundoku app, not a browser."))
}

/** In a browser, whether the address has `?name` (`?setup`, `&portable`), to see what the stand-ins don't show. */
function addressHas(name: string): boolean {
  return new URLSearchParams(window.location.search).has(name)
}

/** In a browser, the number in `?name=5000`, or null without one. */
function addressNumber(name: string): number | null {
  const value = Number(new URLSearchParams(window.location.search).get(name))
  return Number.isInteger(value) && value > 0 ? value : null
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

/** Opens the download folder in File Explorer, making it if it isn't there yet. Rejects with Windows' reason. */
export async function openDownloadFolder(): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("open_download_folder")
}

/** Asks where to save a backup, suggesting `fileName`. Resolves to its path, or null if the user cancelled. */
export async function chooseBackupDestination(title: string, fileName: string, filterName: string): Promise<string | null> {
  if (!isTauri()) return null
  return save({ title, defaultPath: fileName, filters: [{ name: filterName, extensions: ["zip"] }] })
}

/** Asks for a backup to restore. Resolves to its path, or null if the user cancelled. */
export async function pickBackup(title: string, filterName: string): Promise<string | null> {
  if (!isTauri()) return null
  return open({ title, multiple: false, directory: false, filters: [{ name: filterName, extensions: ["zip"] }] })
}

/** Saves the settings, presets, folder rules and history in a backup file. Rejects with a BackupError. */
export async function saveBackup(path: string): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("save_backup", { path })
}

/**
 * Replaces the settings, presets, folder rules and history with a backup's, after saving the current ones as an
 * automatic backup in the data folder, then restarts Tsundoku so everything reads what was restored. Rejects with a
 * BackupError, having changed nothing.
 */
export async function restoreBackup(path: string): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("restore_backup", { path })
  await relaunch()
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

/**
 * Updates Tsundoku's own copy of gallery-dl to its latest release, downloading it only when it's newer, once nothing is
 * running it. Progress arrives through `onToolProgress`.
 */
export function updateGalleryDl(): Promise<ToolStatus> {
  if (!isTauri()) return outsideTauri()
  return invoke<ToolStatus>("update_gallery_dl")
}

/**
 * Settings → Tools' Update All: updates each of Tsundoku's own copies in turn, leaving the ones the user chose, and
 * says what it did to each. Progress arrives through `onToolProgress`.
 */
export function updateAllTools(): Promise<ToolUpdateOutcome[]> {
  if (!isTauri()) return outsideTauri()
  return invoke<ToolUpdateOutcome[]>("update_all_tools")
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
/** What a link is: one video or post, or what a channel, playlist or account holds. Rejects with a PreviewError. */
/** What a link is, read with `password` when it needs one (else the one the history keeps for it). Rejects with a
 * PreviewError, `passwordNeeded` when it needs a password, `wrong` when the one given didn't open it. */
export function fetchPreview(link: string, password: string | null = null): Promise<LinkPreview> {
  // `?folder=2000` gives a MEGA link that many files, to check how the list copes.
  if (!isTauri()) return standIns.fetchPreview(link, password, addressNumber("folder"))
  return invoke<LinkPreview>("fetch_preview", { link, password })
}

export async function cancelPreview(): Promise<void> {
  if (!isTauri()) return
  await invoke("cancel_preview")
}

/** Whether a link is a channel, playlist or account, from its address alone, or null. */
export function listingKind(link: string): Promise<ListingKind | null> {
  if (!isTauri()) return Promise.resolve(standIns.listingKind(link))
  return invoke<ListingKind | null>("listing_kind", { link })
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

/** Adds a preset at the end of the list, set up as Best Video ships. */
export function addPreset(name: string): Promise<Preset> {
  if (!isTauri()) return outsideTauri()
  return invoke<Preset>("add_preset", { name })
}

/** Moves a preset to `index` in the list; the default stays first. */
export function movePreset(id: string, index: number): Promise<PresetsOverview> {
  if (!isTauri()) return outsideTauri()
  return invoke<PresetsOverview>("move_preset", { id, index })
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
  // `?empty` shows an empty queue, with its ways to add links.
  if (!isTauri()) return Promise.resolve(addressHas("empty") ? [] : standIns.queueItems)
  return invoke<QueueItem[]>("queue_items")
}

/** Adds a video to the download queue. Rejects with a DownloadError. */
export function addToQueue(
  video: VideoInfo,
  presetId: string,
  overwrite: boolean,
  folder: string | null = null,
  password: string | null = null,
): Promise<QueueItem> {
  if (!isTauri()) return outsideTauri()
  return invoke<QueueItem>("add_to_queue", { video, presetId, overwrite, folder, password })
}

/** Adds a post of images, with the files its `postFiles` tick (all of them when it lists none), into the folder the
 * rules give it. Rejects with a DownloadError, `nothingPicked` when every file is unticked. */
export function addPostToQueue(
  video: VideoInfo,
  folder: string | null = null,
  password: string | null = null,
): Promise<QueueItem> {
  if (!isTauri()) return outsideTauri()
  return invoke<QueueItem>("add_post_to_queue", { video, folder, password })
}

/** Saves a download that isn't running in a folder picked for it, or back in the usual one (`null`). A stopped one
 * starts over, since its partial files are in the old folder. */
export function setQueueFolder(id: number, folder: string | null): Promise<QueueItem> {
  if (!isTauri()) return outsideTauri()
  return invoke<QueueItem>("set_queue_folder", { id, folder })
}

/** The files of a post its downloads left out, by number, or null when they left none out (or there were none). */
export function getLeftOutFiles(video: VideoInfo): Promise<number[] | null> {
  if (!isTauri()) return Promise.resolve(standIns.leftOutFiles(video))
  return invoke<number[] | null>("left_out_files", { video })
}

/** The files of a shared folder its downloads didn't take, by their place in its list, or null when they took them
 * all (or there were none). */
export function getLeftOutSharedFiles(video: VideoInfo): Promise<number[] | null> {
  if (!isTauri()) return Promise.resolve(standIns.leftOutSharedFiles(video))
  return invoke<number[] | null>("left_out_shared_files", { video })
}

/** Adds the items picked from a channel, playlist or account, each as its own download, and answers how many were
 * added. Rejects with a DownloadError once every other one is in. */
export function addManyToQueue(picks: QueuePick[]): Promise<number> {
  if (!isTauri()) return outsideTauri()
  return invoke<number>("add_many_to_queue", { picks })
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
export function getDownloadDestination(
  video: VideoInfo,
  presetId: string | null,
  ruleId: string | null = null,
): Promise<Destination> {
  if (!isTauri()) return Promise.resolve(standIns.destination(video, ruleId))
  return invoke<Destination>("download_destination", { video, presetId, ruleId })
}

/** The preset a rule picks for a video, and which rule: null when no rule picks one, and for a post or a file host's
 * files. */
export function getRulePreset(video: VideoInfo, ruleId: string | null = null): Promise<RulePreset | null> {
  if (!isTauri()) return Promise.resolve(standIns.rulePreset(video, ruleId))
  return invoke<RulePreset | null>("rule_preset", { video, ruleId })
}

/** Whether a rule makes copied and sent links of a video start once read, and which rule: null when Settings' Download
 * Right Away decides. `presetId` is the preset it would start with, none for a post or a file host's files. */
export function getRuleRightAway(video: VideoInfo, presetId: string | null): Promise<RightAwayRule | null> {
  if (!isTauri()) return Promise.resolve(standIns.ruleRightAway(video))
  return invoke<RightAwayRule | null>("rule_download_right_away", { video, presetId })
}

/** Whether a rule unpacks a video's archives, and where, and which rule: null when Settings → Archives decides.
 * `presetId` is the preset it would start with, none for a post or a file host's files. */
export function getRuleUnpacking(video: VideoInfo, presetId: string | null): Promise<UnpackingRule | null> {
  if (!isTauri()) return Promise.resolve(standIns.ruleUnpacking(video))
  return invoke<UnpackingRule | null>("rule_unpacking", { video, presetId })
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

/** Try Now: starts a download that waits to try again, or for its site, as soon as there's room. */
export async function tryDownloadNow(id: number): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("try_download_now", { id })
}

/** Enter Password, for a download that waits for one: it starts with it. Rejects with a DownloadError. */
export async function enterDownloadPassword(id: number, password: string): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("enter_download_password", { id, password })
}

/** Open Check: shows the check a download waits for (a site's check that you're a person) in Tsundoku's check window,
 * where the person passes it and starts the download on the site's page. */
export async function openCheckWindow(id: number): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("open_check_window", { id })
}

/** Removes a download that isn't done, with its partial files. */
export async function cancelDownload(id: number): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("cancel_download", { id })
}

/** Pause All: stops every running download, keeping its partial files, and holds every waiting one. */
export async function stopAllDownloads(): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("stop_all_downloads")
}

export async function resumeAllDownloads(): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("resume_all_downloads")
}

export async function retryFailedDownloads(): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("retry_failed_downloads")
}

/** Moves a waiting download to the front of the line, so it starts before the others waiting. */
export async function downloadNext(id: number): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("download_next", { id })
}

/** Removes the finished downloads and answers how many, which restoreClearedDownloads puts back. */
export function clearFinishedDownloads(): Promise<number> {
  if (!isTauri()) return outsideTauri()
  return invoke<number>("clear_finished_downloads")
}

/** Removes one finished download, which restoreClearedDownloads puts back. The history keeps it. */
export async function removeFinishedDownload(id: number): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("remove_finished_download", { id })
}

/** Undo for the last Clear Finished, or the last finished download removed: its downloads go back where they were, unless the queue has their video again. */
export async function restoreClearedDownloads(): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("restore_cleared_downloads")
}

/** Opens a download's log in the default text editor. Rejects with a DownloadError. */
export async function openDownloadLog(id: number): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("open_download_log", { id })
}

/**
 * What a bug report about a failed download needs, as plain text with the user's name blanked out: the versions, the
 * link, the error as the row shows it and the end of its log. Takes a moment. Rejects with a DownloadError.
 */
export function getDownloadDetails(id: number, shownError: ShownError): Promise<string> {
  if (!isTauri()) return outsideTauri()
  return invoke<string>("download_details", { id, shownError })
}

export function onQueueChanged(handler: (items: QueueItem[]) => void): Promise<UnlistenFn> {
  if (!isTauri()) return Promise.resolve(() => {})
  return listen<QueueItem[]>(QUEUE_CHANGED_EVENT, (event) => handler(event.payload))
}

/** Links waiting to be started, oldest first. */
export function getPendingLinks(): Promise<PendingLink[]> {
  // `?empty` shows an empty Pending, with its ways to add links.
  if (!isTauri()) return Promise.resolve(addressHas("empty") ? [] : standIns.pendingLinks)
  return invoke<PendingLink[]>("pending_links")
}

/** The links in pasted or dropped text, each once, so one link can be told from several. */
export function linksInText(text: string): Promise<string[]> {
  if (!isTauri()) return Promise.resolve(standIns.linksInText(text))
  return invoke<string[]>("links_in_text", { text })
}

/** Whether a dropped file can be read for links, from its name and size, before it's read. Rejects with a
 * LinkFileError. */
export function checkLinkFile(name: string, size: number): Promise<void> {
  if (!isTauri()) return Promise.resolve().then(() => standIns.checkLinkFile(name, size))
  return invoke<void>("check_link_file", { name, size })
}

/** The links in a dropped file, read by the core from its bytes. Rejects with a LinkFileError. */
export function linksInLinkFile(name: string, bytes: Uint8Array): Promise<string[]> {
  if (!isTauri()) return Promise.resolve().then(() => standIns.linksInLinkFile(name, bytes))
  // A header value is ASCII, so the name goes as JSON with every character past ASCII escaped.
  const asciiName = JSON.stringify(name).replace(
    /[\u0080-￿]/g,
    (character) => `\\u${character.charCodeAt(0).toString(16).padStart(4, "0")}`,
  )
  return invoke<string[]>("links_in_link_file", bytes, { headers: { "tsundoku-file-name": asciiName } })
}

/** The links in a file chosen on this PC, read by the core. Rejects with a LinkFileError. */
export function linksInFileAt(path: string): Promise<string[]> {
  if (!isTauri()) return outsideTauri()
  return invoke<string[]>("links_in_file_at", { path })
}

/** Files of links to choose on this PC, several at once. None when the dialog is cancelled, and in a browser. */
export async function pickLinkFiles(title: string, filterName: string, extensions: string[]): Promise<string[]> {
  if (!isTauri()) return []
  return (await open({ title, multiple: true, directory: false, filters: [{ name: filterName, extensions }] })) ?? []
}

/** The text on the clipboard, or null when it holds none. In a browser, the browser's clipboard. */
export async function clipboardText(): Promise<string | null> {
  if (!isTauri()) return navigator.clipboard.readText().catch(() => null)
  return invoke<string | null>("clipboard_text")
}

/** Adds the links read from dropped files: they're checked as copied links are, and those with nothing to download
 * leave once read. Rejects with a DownloadError. */
export function addPendingLinksFromFiles(links: string[]): Promise<AddedLinks> {
  if (!isTauri()) return outsideTauri()
  return invoke<AddedLinks>("add_pending_links_from_files", { links })
}

/** Adds links to the pending ones, which are then read one at a time. Rejects with a DownloadError. */
export function addPendingLinks(links: string[]): Promise<AddedLinks> {
  if (!isTauri()) return outsideTauri()
  return invoke<AddedLinks>("add_pending_links", { links })
}

/** Removes links and answers how many, which restoreRemovedPendingLinks puts back. */
export function removePendingLinks(ids: number[]): Promise<number> {
  if (!isTauri()) return outsideTauri()
  return invoke<number>("remove_pending_links", { ids })
}

/** Undo for the last removal: its links go back as they were, unless they're here again by another way. */
export async function restoreRemovedPendingLinks(): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("restore_removed_pending_links")
}

/** Reads a link that couldn't be read again, after a fix such as signing in. */
export async function readPendingLinkAgain(id: number): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("read_pending_link_again", { id })
}

/** A pending link that lists several items was dealt with in the preview card's picker (items picked from it joined
 * the queue, or it's watched now): it leaves Pending. Rejects with a DownloadError. */
export async function pendingLinkPicked(id: number): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("pending_link_picked", { id })
}

/** Unlock, for a pending link that needs a password: it's read again with it. Rejects with a DownloadError. */
export async function unlockPendingLink(id: number, password: string): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("unlock_pending_link", { id, password })
}

/** Picks which files of a read post one link downloads, by their number in the post. Rejects with a DownloadError. */
export async function setPendingPostFiles(id: number, picked: number[]): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("set_pending_post_files", { id, picked })
}

/** Picks which files of a read shared folder one link downloads, by their place in its list, from 0. Rejects with a
 * DownloadError. */
export async function setPendingSharedFiles(id: number, picked: number[]): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("set_pending_shared_files", { id, picked })
}

/** Picks one link's preset, or null for the one picked for them all. */
/** Picks one pending link's folder, or `null` for the one the rules give. */
export async function setPendingFolder(id: number, folder: string | null): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("set_pending_folder", { id, folder })
}

/** Picks the rule that decides first what it sets for one link (its folder, preset and file name), or null for the
 * matching rules. */
export async function setPendingRule(id: number, ruleId: string | null): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("set_pending_rule", { id, ruleId })
}

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

/** Whether Click'n'Load buttons can send links here: listening, off, or the port taken by another app. In a browser,
 * `?portTaken` shows the port taken. */
export function getClickNLoadStatus(): Promise<ClickNLoadStatus> {
  if (!isTauri()) {
    return Promise.resolve({
      ...standIns.clickNLoadStatus,
      state: addressHas("portTaken") ? { kind: "portTaken" } : standIns.clickNLoadStatus.state,
    })
  }
  return invoke<ClickNLoadStatus>("click_n_load_status")
}

export function onClickNLoadChanged(handler: (status: ClickNLoadStatus) => void): Promise<UnlistenFn> {
  if (!isTauri()) return Promise.resolve(() => {})
  return listen<ClickNLoadStatus>(CLICK_N_LOAD_CHANGED_EVENT, (event) => handler(event.payload))
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

/** Opens Windows' color picker on `initial` (`#rrggbb`) and resolves to the color chosen, or null when it was cancelled.
 * WebView2 doesn't open a picker for a color input; in a browser, one does. */
export function pickColor(initial: string): Promise<string | null> {
  if (!isTauri()) return pickColorInBrowser(initial)
  return invoke<string | null>("pick_color", { initial })
}

function pickColorInBrowser(initial: string): Promise<string | null> {
  return new Promise((resolve) => {
    const input = document.createElement("input")
    input.type = "color"
    input.value = initial
    // The native change event fires once a color is chosen; React-style input events fire while the picker is open.
    input.addEventListener("change", () => resolve(input.value), { once: true })
    input.addEventListener("cancel", () => resolve(null), { once: true })
    input.click()
  })
}

/** The watched channels, playlists and accounts, with their new and skipped items. `?empty` shows none. */
export function getWatchedSources(): Promise<WatchedSource[]> {
  if (!isTauri()) return Promise.resolve(addressHas("empty") ? [] : standIns.watchedSources())
  return invoke<WatchedSource[]>("watched_sources")
}

/** Watch, from the preview card's picker. Resolves to the new source's ID. */
export function addWatchedSource(source: NewWatchedSource): Promise<number> {
  if (!isTauri()) return outsideTauri()
  return invoke<number>("add_watched_source", { source })
}

/** A source's Save In and Videos Start As, each null for the rules' and the default. */
export async function editWatchedSource(id: number, folder: string | null, presetId: string | null): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("edit_watched_source", { id, folder, presetId })
}

export async function removeWatchedSource(id: number): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("remove_watched_source", { id })
}

/** Check Now: runs in the background; the source's `checking` says when it's done. */
export async function checkWatchedSource(id: number): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("check_watched_source", { id })
}

/** Queues these items of a source; with `skipRest`, its other new items are skipped. Resolves to how many were queued. */
export function downloadWatchedItems(id: number, items: ItemKey[], skipRest: boolean): Promise<number> {
  if (!isTauri()) return outsideTauri()
  return invoke<number>("download_watched_items", { id, items, skipRest })
}

/** Skip All: every new item of the source is skipped. */
export async function skipWatchedItems(id: number): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("skip_watched_items", { id })
}

export function onWatchedChanged(handler: (sources: WatchedSource[]) => void): Promise<UnlistenFn> {
  if (!isTauri()) return Promise.resolve(() => {})
  return listen<WatchedSource[]>(WATCHED_CHANGED_EVENT, (event) => handler(event.payload))
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

/** One page of the history matching a search, filters and a sort, from the `offset`th video on, each with its latest
 * download, and how many match in all. In a browser, `?history=5000` gives that many stand-ins. */
export function getHistoryPage(query: HistoryQuery, offset: number, limit: number): Promise<HistoryPage> {
  if (!isTauri()) return Promise.resolve(standIns.historyPage(query, offset, limit, addressNumber("history")))
  return invoke<HistoryPage>("history_page", { query, offset, limit })
}

/** Where the window reads a thumbnail kept in the data folder (`HistoryVideo.thumbnailFile`): the app's `thumbnail`
 * protocol. None in a browser, where the site's picture shows instead. */
export function keptThumbnailUrl(fileName: string): string | null {
  if (!isTauri()) return null
  return convertFileSrc(fileName, "thumbnail")
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

/** The IDs of every video a query matches, for ticking them all. */
export function getHistoryIds(query: HistoryQuery): Promise<number[]> {
  if (!isTauri()) return Promise.resolve(standIns.historyIds(query, addressNumber("history")))
  return invoke<number[]>("history_ids", { query })
}

/** Forgets several videos, as forgetVideo does each, and resolves to how many were in the history. */
export function forgetVideos(videoIds: number[]): Promise<number> {
  if (!isTauri()) return outsideTauri()
  return invoke<number>("forget_videos", { videoIds })
}

/** History's Move Files…: moves the videos' files that are still there into `folder`, keeping each download's
 * subfolders, and says how it went. */
/** The end of a download's log, for the Queue's log viewer. */
export function getDownloadLog(id: number): Promise<DownloadLog> {
  if (!isTauri()) return Promise.resolve(standIns.downloadLog)
  return invoke<DownloadLog>("download_log", { id })
}

export function moveHistoryFiles(videoIds: number[], folder: string): Promise<MovedFiles> {
  if (!isTauri()) return outsideTauri()
  return invoke<MovedFiles>("move_history_files", { videoIds, folder })
}

/** How many folders hold the videos' files that are still there. */
export function countHistoryFolders(videoIds: number[]): Promise<number> {
  if (!isTauri()) return Promise.resolve(videoIds.length === 0 ? 0 : Math.min(videoIds.length, 7))
  return invoke<number>("history_folder_count", { videoIds })
}

/** Opens each folder holding the videos' files once, with those files selected. Rejects with a HistoryError. */
export async function showVideosInFolders(videoIds: number[]): Promise<void> {
  if (!isTauri()) return outsideTauri()
  await invoke("show_videos_in_folders", { videoIds })
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

/** How the Pending and Queue views group and sort their lists. */
export function getListArrangements(): Promise<ListArrangements> {
  if (!isTauri()) return Promise.resolve(standIns.listArrangements)
  return invoke<ListArrangements>("list_arrangements")
}

/** Saves how the Pending and Queue views arrange their lists, and resolves to them as saved. Rejects with a
 * DownloadError. In a browser, the choice lasts until the page reloads, to check each arrangement's layout. */
export function saveListArrangements(arrangements: ListArrangements): Promise<ListArrangements> {
  if (!isTauri()) return Promise.resolve(arrangements)
  return invoke<ListArrangements>("save_list_arrangements", { arrangements })
}

/** How the pickers show a listing's items and a post's files. */
export function getPickerLayouts(): Promise<PickerLayouts> {
  if (!isTauri()) return Promise.resolve(standIns.pickerLayouts)
  return invoke<PickerLayouts>("picker_layouts")
}

/** Saves how the pickers show what they offer, and resolves to it as saved. Rejects with a DownloadError. In a
 * browser, the choice lasts until the page reloads. */
export function savePickerLayouts(layouts: PickerLayouts): Promise<PickerLayouts> {
  if (!isTauri()) return Promise.resolve(layouts)
  return invoke<PickerLayouts>("save_picker_layouts", { layouts })
}

/** A newer release of Tsundoku, found on GitHub. */
export interface AppUpdate {
  version: string
  /** What changed: the release's CHANGELOG section, in Markdown, or null when it came without one. */
  notes: string | null
  /** The release's page on GitHub. */
  releasePage: string
  /** Downloads the update, reporting bytes received, then installs it and restarts the app. */
  install: (onProgress: (received: number, total: number | null) => void) => Promise<void>
}

/** Where a release of Tsundoku is on GitHub. */
function releasePageOf(version: string): string {
  return `https://github.com/Fuzen777/Tsundoku/releases/tag/v${version}`
}

/**
 * Asks GitHub for a newer release, through the updater's latest.json, whose notes the release run takes from that
 * version's CHANGELOG section. Resolves to null when there's none, and always in development. Rejects when GitHub
 * can't be asked, as while the repository is private. In a browser, `?update` shows one with notes and
 * `?update=bare` one without, which can't be installed.
 */
export async function checkForAppUpdate(): Promise<AppUpdate | null> {
  if (!isTauri()) {
    if (!addressHas("update")) return null
    const bare = new URLSearchParams(window.location.search).get("update") === "bare"
    const { version, notes } = standIns.appUpdate
    return { version, notes: bare ? null : notes, releasePage: releasePageOf(version), install: outsideTauri }
  }
  if (import.meta.env.DEV) return null
  const update = await check()
  if (!update) return null
  return {
    version: update.version,
    notes: update.body?.trim() || null,
    releasePage: releasePageOf(update.version),
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

/** What changed since the version that ran last, newest first, for the What's New card; none on a first install or
 * once it's closed. In a browser, `?whatsnew` shows stand-ins. */
export function getWhatsNew(): Promise<ChangelogSection[]> {
  if (!isTauri()) return Promise.resolve(addressHas("whatsnew") ? standIns.whatsNew : [])
  return invoke<ChangelogSection[]>("whats_new")
}

/** Closes the What's New card until a newer version runs. Rejects with a DownloadError. */
export async function dismissWhatsNew(): Promise<void> {
  if (!isTauri()) return
  await invoke("dismiss_whats_new")
}

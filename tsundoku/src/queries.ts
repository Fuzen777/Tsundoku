import { useCallback, useState } from "react"
import {
  keepPreviousData,
  useInfiniteQuery,
  useQueries,
  useQuery,
  useQueryClient,
  type QueryObserverResult,
} from "@tanstack/react-query"
import {
  checkForAppUpdate,
  detectTools,
  dismissWhatsNew,
  getAddingOptions,
  getClickNLoadStatus,
  getAppInfo,
  getAppearanceOptions,
  getBackgroundOptions,
  getDefaultDownloadFolder,
  getDownloadDestination,
  getDownloadFiles,
  getDownloadLog,
  getDownloadOptions,
  getLeftOutFiles,
  getLeftOutSharedFiles,
  getFirstRunOptions,
  getFolderRules,
  getListArrangements,
  getSupportedSites,
  getHistoryFilters,
  getHistoryPage,
  getVideoDownloads,
  getPresetArguments,
  getPendingLinks,
  getPresetsOverview,
  getQueueItems,
  getRulePreset,
  getWatchedSources,
  getRuleRightAway,
  getRuleUnpacking,
  getToolDownloads,
  getToolsNeedSetup,
  getToolsOverview,
  getWhatsNew,
  queryKeys,
  saveAddingOptions,
  saveAppearanceOptions,
  saveBackgroundOptions,
  saveDownloadOptions,
  saveListArrangements,
} from "./api"
import { ruleGroup, STORED_ORDER, type GroupPlace } from "./arrange"
import { ruleLabel } from "./views/rules/ruleLabels"
import type { AddingSettings } from "./bindings/AddingSettings"
import type { AppearanceSettings } from "./bindings/AppearanceSettings"
import type { BackgroundSettings } from "./bindings/BackgroundSettings"
import type { Destination } from "./bindings/Destination"
import type { DownloadSettings } from "./bindings/DownloadSettings"
import type { HistoryQuery } from "./bindings/HistoryQuery"
import type { ListArrangement } from "./bindings/ListArrangement"
import type { ListArrangements } from "./bindings/ListArrangements"
import type { Preset } from "./bindings/Preset"
import type { PresetsOverview } from "./bindings/PresetsOverview"
import type { RulePreset } from "./bindings/RulePreset"
import type { ToolCaches } from "./bindings/ToolCaches"
import type { ToolStatus } from "./bindings/ToolStatus"
import type { ToolsOverview } from "./bindings/ToolsOverview"
import type { UpdateChannel } from "./bindings/UpdateChannel"
import type { VideoInfo } from "./bindings/VideoInfo"
import { describeDownloadError, type ErrorDescription } from "./errors"
import { applyAppearance } from "./theme"

/*
  React Query hooks over api.ts. The tool queries never go stale on their own: they change only through the app's
  actions, which update them here, or the core's tools-changed event (main.tsx).
*/

/** The running app's version and backdrop. main.tsx reads it before the first render, so it's always there. */
export function useAppInfo() {
  return useQuery({ queryKey: queryKeys.appInfo, queryFn: getAppInfo, staleTime: Infinity })
}

/** Whether to show the first-run setup. main.tsx reads it before the first render. */
export function useToolsNeedSetup() {
  return useQuery({ queryKey: queryKeys.toolsNeedSetup, queryFn: getToolsNeedSetup, staleTime: Infinity })
}

/** The first-run choices. Saving them sets `needed` to false here, which moves the setup on to the tools. */
export function useFirstRunOptions() {
  return useQuery({ queryKey: queryKeys.firstRunOptions, queryFn: getFirstRunOptions, staleTime: Infinity })
}

export function useToolsOverview() {
  return useQuery({ queryKey: queryKeys.toolsOverview, queryFn: getToolsOverview, staleTime: Infinity })
}

export function useDetectedTools() {
  return useQuery({ queryKey: queryKeys.detectedTools, queryFn: detectTools, staleTime: Infinity })
}

export function useToolDownloads() {
  return useQuery({ queryKey: queryKeys.toolDownloads, queryFn: getToolDownloads, staleTime: Infinity })
}

/** Leaves the first-run setup for the app, with the tools' statuses read afresh. */
export function useFinishSetup() {
  const queryClient = useQueryClient()
  return useCallback(() => {
    void queryClient.invalidateQueries({ queryKey: queryKeys.toolsOverview })
    queryClient.setQueryData(queryKeys.toolsNeedSetup, false)
  }, [queryClient])
}

/** Puts a tool's status, as an action returned it, into the overview. */
export function useStoreToolStatus() {
  const queryClient = useQueryClient()
  return useCallback(
    (status: ToolStatus) => {
      queryClient.setQueryData<ToolsOverview>(
        queryKeys.toolsOverview,
        (overview) =>
          overview && { ...overview, tools: overview.tools.map((tool) => (tool.id === status.id ? status : tool)) },
      )
    },
    [queryClient],
  )
}

export function useStoreToolCaches() {
  const queryClient = useQueryClient()
  return useCallback(
    (caches: ToolCaches) => {
      queryClient.setQueryData<ToolsOverview>(queryKeys.toolsOverview, (overview) => overview && { ...overview, caches })
    },
    [queryClient],
  )
}

export function useStoreYtDlpChannel() {
  const queryClient = useQueryClient()
  return useCallback(
    (channel: UpdateChannel) => {
      queryClient.setQueryData<ToolsOverview>(
        queryKeys.toolsOverview,
        (overview) => overview && { ...overview, ytDlpChannel: channel },
      )
    },
    [queryClient],
  )
}

export function usePresets() {
  return useQuery({ queryKey: queryKeys.presets, queryFn: getPresetsOverview, staleTime: Infinity })
}

/** Puts the presets, as an action returned them, in place. A preset's folder can change where a download goes. */
export function useStorePresets() {
  const queryClient = useQueryClient()
  return useCallback(
    (overview: PresetsOverview) => {
      queryClient.setQueryData(queryKeys.presets, overview)
      void queryClient.invalidateQueries({ queryKey: queryKeys.downloadDestination })
      // A rule's preset may have been deleted.
      void queryClient.invalidateQueries({ queryKey: queryKeys.rulePreset })
    },
    [queryClient],
  )
}

export function useFolderRules() {
  return useQuery({ queryKey: queryKeys.folderRules, queryFn: getFolderRules, staleTime: Infinity })
}

/** Where a download would go, kept while the next answer loads so the preview card doesn't flicker. */
export function useDownloadDestination(
  video: VideoInfo,
  presetId: string | null,
  enabled = true,
  ruleId: string | null = null,
) {
  return useQuery(destinationQuery({ video, presetId, enabled, ruleId }))
}

/** A download whose folder is asked: the video, the preset it would start with (none for a post or a file host's
 * files), and the rule picked for it, if any. */
export interface DestinationRequest {
  video: VideoInfo
  presetId: string | null
  enabled: boolean
  ruleId?: string | null
}

/** Where several downloads would go, in the order asked, each undefined until it's known. They share their answers
 * with `useDownloadDestination`. */
export function useDownloadDestinations(requests: readonly DestinationRequest[]): (Destination | undefined)[] {
  return useQueries({ queries: requests.map(destinationQuery), combine: destinationsOf })
}

function destinationQuery({ video, presetId, enabled, ruleId = null }: DestinationRequest) {
  return {
    queryKey: [...queryKeys.downloadDestination, video.site, video.videoId, presetId, ruleId],
    queryFn: () => getDownloadDestination(video, presetId, ruleId),
    placeholderData: keepPreviousData,
    enabled,
  }
}

function destinationsOf(results: QueryObserverResult<Destination>[]): (Destination | undefined)[] {
  return results.map((result) => result.data)
}

/** The files of a post its downloads left out (`getLeftOutFiles`), asked only for a post downloaded before. */
export function useLeftOutFiles(video: VideoInfo, enabled: boolean) {
  return useQuery({
    queryKey: [...queryKeys.leftOutFiles, video.site, video.videoId],
    queryFn: () => getLeftOutFiles(video),
    enabled,
  })
}

/** The files of a shared folder its downloads didn't take (`getLeftOutSharedFiles`), asked only for a folder downloaded
 * before. */
export function useLeftOutSharedFiles(video: VideoInfo, enabled: boolean) {
  return useQuery({
    queryKey: [...queryKeys.leftOutSharedFiles, video.site, video.videoId],
    queryFn: () => getLeftOutSharedFiles(video),
    enabled,
  })
}

/** Whether a rule makes copied and sent links of a video start once read, and which rule: null when Settings decides. */
export function useRuleRightAway(video: VideoInfo | null, presetId: string | null, enabled: boolean) {
  return useQuery({
    queryKey: [...queryKeys.ruleRightAway, video?.site, video?.videoId, presetId],
    queryFn: () => (video ? getRuleRightAway(video, presetId) : null),
    enabled: enabled && video !== null,
  })
}

/** Whether a rule unpacks a video's archives, and where, and which rule: null when Settings → Archives decides. */
export function useRuleUnpacking(video: VideoInfo | null, presetId: string | null, enabled: boolean) {
  return useQuery({
    queryKey: [...queryKeys.ruleUnpacking, video?.site, video?.videoId, presetId],
    queryFn: () => (video ? getRuleUnpacking(video, presetId) : null),
    enabled: enabled && video !== null,
  })
}

/** The watched sources, kept current by `watched-changed`. */
export function useWatchedSources() {
  return useQuery({ queryKey: queryKeys.watched, queryFn: getWatchedSources, staleTime: Infinity })
}

/** The preset a rule picks for a video, and which rule: null when none does. Not asked for a post or a file host's
 * files, which have no preset. */
export function useRulePreset(video: VideoInfo) {
  return useQuery(rulePresetQuery(video))
}

/** The presets rules pick for several videos, in the order asked, each with the rule picked for it in `ruleIds`
 * first, if any: undefined until known, null when no rule picks one. They share their answers with `useRulePreset`. */
export function useRulePresets(
  videos: readonly VideoInfo[],
  ruleIds: readonly (string | null)[] = [],
): (RulePreset | null | undefined)[] {
  return useQueries({
    queries: videos.map((video, index) => rulePresetQuery(video, ruleIds[index] ?? null)),
    combine: rulePresetsOf,
  })
}

function rulePresetQuery(video: VideoInfo, ruleId: string | null = null) {
  return {
    queryKey: [...queryKeys.rulePreset, video.site, video.videoId, ruleId],
    queryFn: () => getRulePreset(video, ruleId),
    enabled: !video.post && !video.files,
  }
}

function rulePresetsOf(results: QueryObserverResult<RulePreset | null>[]): (RulePreset | null | undefined)[] {
  return results.map((result) => result.data)
}

/** The yt-dlp arguments a preset gives, kept while the next ones load so the editor doesn't flicker. */
export function usePresetArguments(preset: Preset) {
  return useQuery({
    queryKey: [...queryKeys.presetArguments, preset],
    queryFn: () => getPresetArguments(preset),
    placeholderData: keepPreviousData,
    retry: false,
  })
}

/** The download queue. The core's queue-changed events keep it current (queueEvents.ts). */
export function useQueue() {
  return useQuery({ queryKey: queryKeys.queue, queryFn: getQueueItems, staleTime: Infinity })
}

/** Links waiting to be started. The core's pending-changed events keep them current (queueEvents.ts). */
export function usePendingLinks() {
  return useQuery({ queryKey: queryKeys.pending, queryFn: getPendingLinks, staleTime: Infinity })
}

/** How the Pending or Queue view groups and sorts its list (the stored order until it's read), and a change to it,
 * which takes effect at once and is saved, or undone with the error if saving failed. */
export function useListArrangement(list: keyof ListArrangements) {
  const queryClient = useQueryClient()
  const { data: arrangements } = useQuery({
    queryKey: queryKeys.listArrangements,
    queryFn: getListArrangements,
    staleTime: Infinity,
  })
  const [error, setError] = useState<ErrorDescription | null>(null)
  const change = async (changes: Partial<ListArrangement>) => {
    if (!arrangements) return
    const changed = { ...arrangements, [list]: { ...arrangements[list], ...changes } }
    setError(null)
    queryClient.setQueryData(queryKeys.listArrangements, changed)
    try {
      await saveListArrangements(changed)
    } catch (caught) {
      queryClient.setQueryData(queryKeys.listArrangements, arrangements)
      setError(describeDownloadError(caught))
    }
  }
  return { arrangement: arrangements?.[list] ?? STORED_ORDER, ready: arrangements !== undefined, change, error }
}

/** How many videos the history view reads at a time. */
const HISTORY_PAGE_SIZE = 100

/** The history view's results, read a page at a time as the list scrolls, the last results kept while new ones load.
 * Refetched when a download finishes or a video is forgotten (queueEvents.ts). */
export function useHistory(query: HistoryQuery) {
  return useInfiniteQuery({
    queryKey: [...queryKeys.history, query],
    queryFn: ({ pageParam }) => getHistoryPage(query, pageParam, HISTORY_PAGE_SIZE),
    initialPageParam: 0,
    getNextPageParam: (lastPage, pages) => {
      const loaded = pages.reduce((count, page) => count + page.videos.length, 0)
      return lastPage.videos.length === HISTORY_PAGE_SIZE && loaded < lastPage.total ? loaded : undefined
    },
    placeholderData: keepPreviousData,
  })
}

export function useHistoryFilters() {
  return useQuery({ queryKey: queryKeys.historyFilters, queryFn: getHistoryFilters })
}

/** Every download of a video, loaded when its row is expanded. */
/** A download's log, read again every second while `live` (it's running). */
export function useDownloadLog(downloadId: number, live: boolean) {
  return useQuery({
    queryKey: [...queryKeys.downloadLog, downloadId],
    queryFn: () => getDownloadLog(downloadId),
    refetchInterval: live ? 1000 : false,
  })
}

export function useDownloadFiles(downloadId: number, enabled: boolean) {
  return useQuery({
    queryKey: [...queryKeys.downloadFiles, downloadId],
    queryFn: () => getDownloadFiles(downloadId),
    enabled,
  })
}

export function useVideoDownloads(videoId: number, enabled: boolean) {
  return useQuery({
    queryKey: [...queryKeys.videoDownloads, videoId],
    queryFn: () => getVideoDownloads(videoId),
    enabled,
  })
}

/** The supported sites, read again when Settings opens after a minute, since sign-ins change in the browser. */
export function useSupportedSites() {
  return useQuery({ queryKey: queryKeys.supportedSites, queryFn: getSupportedSites, staleTime: 60_000 })
}

export function useDefaultDownloadFolder() {
  return useQuery({ queryKey: queryKeys.defaultDownloadFolder, queryFn: getDefaultDownloadFolder, staleTime: Infinity })
}

export function useDownloadOptions() {
  return useQuery({ queryKey: queryKeys.downloadOptions, queryFn: getDownloadOptions, staleTime: Infinity })
}

/** Saves download options as they change (Settings → Downloads, Archives, Posts of Images, Pacing, Cookies), with the
 * error if it failed. Each place shows its own error. `save` answers whether it saved. */
export function useSaveDownloadOptions() {
  const queryClient = useQueryClient()
  const { data: options } = useDownloadOptions()
  const [error, setError] = useState<ErrorDescription | null>(null)
  const save = async (changes: Partial<DownloadSettings>) => {
    if (!options) return false
    setError(null)
    try {
      queryClient.setQueryData(queryKeys.downloadOptions, await saveDownloadOptions({ ...options, ...changes }))
      return true
    } catch (caught) {
      setError(describeDownloadError(caught))
      return false
    }
  }
  return { options, save, error }
}

export function useAddingOptions() {
  return useQuery({ queryKey: queryKeys.addingOptions, queryFn: getAddingOptions, staleTime: Infinity })
}

/** Whether Click'n'Load buttons can send links here; the app's event keeps it current (main.tsx). */
export function useClickNLoadStatus() {
  return useQuery({ queryKey: queryKeys.clickNLoad, queryFn: getClickNLoadStatus, staleTime: Infinity })
}

/** Saves changes to how links get added, from Settings or the Pending view's switch, with the error if it failed. */
export function useSaveAddingOptions() {
  const queryClient = useQueryClient()
  const { data: options } = useAddingOptions()
  const [error, setError] = useState<ErrorDescription | null>(null)
  const save = async (changes: Partial<AddingSettings>) => {
    if (!options) return
    setError(null)
    try {
      queryClient.setQueryData(queryKeys.addingOptions, await saveAddingOptions({ ...options, ...changes }))
    } catch (caught) {
      setError(describeDownloadError(caught))
    }
  }
  return { options, save, error }
}

export function useBackgroundOptions() {
  return useQuery({ queryKey: queryKeys.backgroundOptions, queryFn: getBackgroundOptions, staleTime: Infinity })
}

/** Saves changes to Settings → Running in the Background, with the error if it failed. */
export function useSaveBackgroundOptions() {
  const queryClient = useQueryClient()
  const { data: options } = useBackgroundOptions()
  const [error, setError] = useState<ErrorDescription | null>(null)
  const save = async (changes: Partial<BackgroundSettings>) => {
    if (!options) return
    setError(null)
    try {
      queryClient.setQueryData(queryKeys.backgroundOptions, await saveBackgroundOptions({ ...options, ...changes }))
    } catch (caught) {
      setError(describeDownloadError(caught))
    }
  }
  return { options, save, error }
}

/** Light, dark or Windows' theme. main.tsx reads it before the first render, so it's always there. */
export function useAppearanceOptions() {
  return useQuery({ queryKey: queryKeys.appearanceOptions, queryFn: getAppearanceOptions, staleTime: Infinity })
}

/** Saves changes to Settings → Appearance, with the error if it failed. The UI takes a new theme at once, and goes
 * back to the saved one if saving fails. */
export function useSaveAppearanceOptions() {
  const queryClient = useQueryClient()
  const { data: options } = useAppearanceOptions()
  const [error, setError] = useState<ErrorDescription | null>(null)
  const save = async (changes: Partial<AppearanceSettings>) => {
    if (!options) return
    const changed = { ...options, ...changes }
    setError(null)
    applyAppearance(changed)
    try {
      queryClient.setQueryData(queryKeys.appearanceOptions, await saveAppearanceOptions(changed))
    } catch (caught) {
      applyAppearance(options)
      setError(describeDownloadError(caught))
    }
  }
  return { options, save, error }
}

/**
 * A newer release of Tsundoku, checked once at startup; Settings → About checks again. Not retried: while the
 * repository is private, GitHub refuses, and that says nothing worth showing. A portable copy doesn't check: the
 * update would install Tsundoku instead of replacing the portable copy.
 */
export function useAppUpdate() {
  const { data: appInfo } = useAppInfo()
  return useQuery({
    queryKey: queryKeys.appUpdate,
    queryFn: checkForAppUpdate,
    staleTime: Infinity,
    retry: false,
    enabled: appInfo !== undefined && !appInfo.portable,
  })
}

/** What changed since the version that ran last, read once at startup, for the What's New card. */
export function useWhatsNew() {
  return useQuery({ queryKey: queryKeys.whatsNew, queryFn: getWhatsNew, staleTime: Infinity })
}

/** Closes the What's New card until a newer version runs, with the error if that couldn't be saved. */
export function useDismissWhatsNew() {
  const queryClient = useQueryClient()
  const [error, setError] = useState<ErrorDescription | null>(null)
  const dismiss = async () => {
    setError(null)
    try {
      await dismissWhatsNew()
      queryClient.setQueryData(queryKeys.whatsNew, [])
    } catch (caught) {
      setError(describeDownloadError(caught))
    }
  }
  return { dismiss, error }
}

/** Groups a download by what picks its folder (`ruleGroup`), once the rules and the download folder are known. */
export function useRuleGroupOf(enabled: boolean): ((destination: Destination) => GroupPlace) | null {
  const { data: rules } = useFolderRules()
  const { data: presets } = usePresets()
  if (!enabled || !rules || !presets) return null
  return (destination) => ruleGroup(destination, rules, presets.defaultFolder)
}

/** What a rule is called by its number (`ruleLabel`): its name, else "Rule 3". */
export function useRuleLabel(): (number: number) => string {
  const { data: rules } = useFolderRules()
  return (number) => ruleLabel(rules, number)
}

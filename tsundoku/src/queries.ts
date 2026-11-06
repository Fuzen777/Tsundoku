import { useCallback } from "react"
import { keepPreviousData, useQuery, useQueryClient } from "@tanstack/react-query"
import {
  checkForAppUpdate,
  detectTools,
  getAppInfo,
  getDefaultDownloadFolder,
  getDownloadDestination,
  getDownloadFiles,
  getDownloadOptions,
  getFolderRules,
  getSupportedSites,
  getHistoryFilters,
  getVideoDownloads,
  searchHistory,
  getPresetArguments,
  getPendingLinks,
  getPresetsOverview,
  getQueueItems,
  getToolDownloads,
  getToolsNeedSetup,
  getToolsOverview,
  queryKeys,
} from "./api"
import type { HistoryQuery } from "./bindings/HistoryQuery"
import type { Preset } from "./bindings/Preset"
import type { PresetsOverview } from "./bindings/PresetsOverview"
import type { ToolStatus } from "./bindings/ToolStatus"
import type { ToolsOverview } from "./bindings/ToolsOverview"
import type { UpdateChannel } from "./bindings/UpdateChannel"
import type { VideoInfo } from "./bindings/VideoInfo"

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
    },
    [queryClient],
  )
}

export function useFolderRules() {
  return useQuery({ queryKey: queryKeys.folderRules, queryFn: getFolderRules, staleTime: Infinity })
}

/** Where a download would go, kept while the next answer loads so the preview card doesn't flicker. */
export function useDownloadDestination(video: VideoInfo, presetId: string | null, enabled = true) {
  return useQuery({
    queryKey: [...queryKeys.downloadDestination, video.site, video.videoId, presetId],
    queryFn: () => getDownloadDestination(video, presetId),
    placeholderData: keepPreviousData,
    enabled,
  })
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

/** The history view's results. Refetched when a download finishes or a video is forgotten (queueEvents.ts). */
export function useHistory(query: HistoryQuery) {
  return useQuery({
    queryKey: [...queryKeys.history, query],
    queryFn: () => searchHistory(query),
    placeholderData: keepPreviousData,
  })
}

export function useHistoryFilters() {
  return useQuery({ queryKey: queryKeys.historyFilters, queryFn: getHistoryFilters })
}

/** Every download of a video, loaded when its row is expanded. */
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

/**
 * A newer release of Tsundoku, checked once at startup; Settings → About checks again. Not retried: while the
 * repository is private, GitHub refuses, and that says nothing worth showing.
 */
export function useAppUpdate() {
  return useQuery({ queryKey: queryKeys.appUpdate, queryFn: checkForAppUpdate, staleTime: Infinity, retry: false })
}


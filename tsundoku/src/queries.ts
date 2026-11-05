import { useCallback } from "react"
import { useQuery, useQueryClient } from "@tanstack/react-query"
import {
  detectTools,
  getAppInfo,
  getToolDownloads,
  getToolsNeedSetup,
  getToolsOverview,
  queryKeys,
} from "./api"
import type { ToolStatus } from "./bindings/ToolStatus"
import type { ToolsOverview } from "./bindings/ToolsOverview"
import type { UpdateChannel } from "./bindings/UpdateChannel"

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

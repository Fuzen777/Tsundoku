import { useQuery } from "@tanstack/react-query"
import { getAppInfo, queryKeys } from "./api"

/** The running app's version and backdrop. main.tsx reads it before the first render, so it's always there. */
export function useAppInfo() {
  return useQuery({ queryKey: queryKeys.appInfo, queryFn: getAppInfo, staleTime: Infinity })
}

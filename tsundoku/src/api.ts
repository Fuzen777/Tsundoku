import { invoke, isTauri } from "@tauri-apps/api/core"
import { getCurrentWindow } from "@tauri-apps/api/window"
import type { AppInfo } from "./bindings/AppInfo"

/*
  The only place the UI calls the Rust side. Types come from src/bindings, generated from the Rust core.
  Outside Tauri (the UI opened in a plain browser to check a layout), each call returns a stand-in instead of failing.
*/

export const queryKeys = {
  appInfo: ["app-info"],
} as const

export function getAppInfo(): Promise<AppInfo> {
  if (!isTauri()) return Promise.resolve({ version: "browser", hasMicaBackdrop: false })
  return invoke<AppInfo>("app_info")
}

/** The window starts hidden (tauri.conf.json); the UI shows it once its first frame is painted. */
export async function showMainWindow(): Promise<void> {
  if (!isTauri()) return
  await getCurrentWindow().show()
}

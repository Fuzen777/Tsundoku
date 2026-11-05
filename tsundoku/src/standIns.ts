import type { AppInfo } from "./bindings/AppInfo"
import type { DetectedTool } from "./bindings/DetectedTool"
import type { ToolDownload } from "./bindings/ToolDownload"
import type { ToolsOverview } from "./bindings/ToolsOverview"
import type { VideoPreview } from "./bindings/VideoPreview"

const previewStandIn: VideoPreview = {
  video: {
    site: "youtube",
    siteName: "youtube.com",
    videoId: "jNQXAC9IVRw",
    url: "https://www.youtube.com/watch?v=jNQXAC9IVRw",
    title: "Me at the zoo",
    uploader: "jawed",
    durationSeconds: 19,
    uploadDate: "2005-04-24",
    thumbnailUrl: "https://i.ytimg.com/vi/jNQXAC9IVRw/hqdefault.jpg",
    bestHeight: 240,
  },
  lastDownload: {
    id: 1,
    preset: "Best Video",
    filePath: String.raw`D:\Videos\Me at the zoo [jNQXAC9IVRw].mkv`,
    fileSize: 791_201,
    format: "240p · av01 + opus",
    downloadedAt: 1_793_876_473,
    fileExists: true,
  },
  missingJsRuntime: true,
}

/** What api.ts returns when the UI runs in a plain browser, to check layouts without the app. */
export const standIns = {
  appInfo: { version: "browser", hasMicaBackdrop: false } satisfies AppInfo,
  toolsOverview: {
    tools: [
      {
        id: "ytDlp",
        setup: "managed",
        path: String.raw`C:\Users\You\AppData\Local\Tsundoku\tools\yt-dlp\yt-dlp.exe`,
        version: "2026.10.26",
        problem: null,
      },
      {
        id: "ffmpeg",
        setup: "external",
        path: String.raw`C:\Program Files\ffmpeg\bin\ffmpeg.exe`,
        version: "9.0.2-full_build-www.gyan.dev",
        problem: null,
      },
      { id: "deno", setup: null, path: null, version: null, problem: null },
    ],
    ytDlpChannel: "stable",
  } satisfies ToolsOverview,
  detectedTools: [
    {
      id: "ytDlp",
      found: {
        path: String.raw`C:\Users\You\AppData\Local\Microsoft\WinGet\Links\yt-dlp.exe`,
        version: "2026.10.26",
        isManaged: false,
      },
    },
    { id: "ffmpeg", found: null },
    { id: "deno", found: null },
  ] satisfies DetectedTool[],
  toolDownloads: [
    { id: "ytDlp", size: 23_575_744 },
    { id: "ffmpeg", size: 90_993_957 },
    { id: "deno", size: 42_917_472 },
  ] satisfies ToolDownload[],
  /** A downloaded video after a short wait, or a failure for a link containing "fail". */
  fetchPreview(link: string): Promise<VideoPreview> {
    return new Promise((resolve, reject) =>
      setTimeout(() => {
        if (link.includes("fail")) {
          reject({ kind: "unavailable", detail: "[youtube] aaaaaaaaaaa: This video is unavailable" })
        } else {
          resolve(previewStandIn)
        }
      }, 800),
    )
  },
}

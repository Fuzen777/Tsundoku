import type { AppInfo } from "./bindings/AppInfo"
import type { DetectedTool } from "./bindings/DetectedTool"
import type { Preset } from "./bindings/Preset"
import type { PresetsOverview } from "./bindings/PresetsOverview"
import type { QueueItem } from "./bindings/QueueItem"
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
  inQueue: false,
  missingJsRuntime: true,
}

function presetStandIn(id: string, name: string, output: Preset["output"]): Preset {
  return {
    id,
    name,
    output,
    fileName: "%(title).150B [%(id)s].%(ext)s",
    folder: null,
    embedMetadata: true,
    embedThumbnail: true,
    embedChapters: output.kind === "video",
    embedSubtitles: false,
    subtitleLanguages: "en.*",
    sponsorBlock: "off",
    extraArguments: "",
  }
}

const presetStandIns: Preset[] = [
  presetStandIn("best-video", "Best Video", { kind: "video", maxHeight: null, container: "mkv" }),
  presetStandIn("1080p-mp4", "1080p MP4", { kind: "video", maxHeight: 1080, container: "mp4" }),
  presetStandIn("best-audio", "Best Audio", { kind: "audio", format: "original" }),
  presetStandIn("audio-mp3", "Audio MP3", { kind: "audio", format: "mp3" }),
]

function queueStandIn(id: number, state: QueueItem["state"], title: string, extra: Partial<QueueItem> = {}): QueueItem {
  return {
    id,
    video: { ...previewStandIn.video, videoId: `video${id}`, title },
    preset: presetStandIns[id % presetStandIns.length],
    folder: String.raw`C:\Users\You\Videos`,
    state,
    overwrite: false,
    addedAt: 1_793_876_473,
    result: null,
    error: null,
    progress: null,
    ...extra,
  }
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
  presetsOverview: {
    presets: presetStandIns,
    defaultId: "best-video",
    builtInIds: presetStandIns.map((preset) => preset.id),
    defaultVideoFolder: String.raw`C:\Users\You\Videos`,
    defaultAudioFolder: String.raw`C:\Users\You\Music`,
  } satisfies PresetsOverview,
  presetArguments: ["-f", "bv*+ba/b", "--merge-output-format", "mkv", "-o", "%(title).150B [%(id)s].%(ext)s"],
  queueItems: [
    queueStandIn(1, "running", "Me at the zoo", {
      progress: {
        stage: "downloadingVideo",
        fraction: 0.42,
        downloadedBytes: 94_000_000,
        totalBytes: 223_779_000,
        speed: 5_400_000,
        eta: 24,
      },
    }),
    queueStandIn(2, "running", "A Merging Example", {
      progress: { stage: "merging", fraction: null, downloadedBytes: null, totalBytes: null, speed: null, eta: null },
    }),
    queueStandIn(3, "waiting", "Next in Line"),
    queueStandIn(4, "stopped", "Stopped Halfway"),
    queueStandIn(5, "failed", "A Video That Failed", {
      error: { kind: "ytDlp", error: { kind: "unavailable", detail: "[youtube] video5: This video is unavailable" } },
    }),
    queueStandIn(6, "done", "Finished Already", {
      result: {
        filePath: String.raw`C:\Users\You\Videos\Finished Already [video6].mkv`,
        fileSize: 52_428_800,
        downloadId: 1,
      },
    }),
  ] satisfies QueueItem[],
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

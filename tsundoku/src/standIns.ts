import type { AddingSettings } from "./bindings/AddingSettings"
import type { AppInfo } from "./bindings/AppInfo"
import type { BackgroundSettings } from "./bindings/BackgroundSettings"
import type { Destination } from "./bindings/Destination"
import type { DetectedTool } from "./bindings/DetectedTool"
import type { DownloadSettings } from "./bindings/DownloadSettings"
import type { FirstRunOptions } from "./bindings/FirstRunOptions"
import type { FolderRule } from "./bindings/FolderRule"
import type { HistoryFile } from "./bindings/HistoryFile"
import type { HistoryFilters } from "./bindings/HistoryFilters"
import type { HistoryQuery } from "./bindings/HistoryQuery"
import type { HistoryVideo } from "./bindings/HistoryVideo"
import type { PastDownload } from "./bindings/PastDownload"
import type { PendingLink } from "./bindings/PendingLink"
import type { Preset } from "./bindings/Preset"
import type { PresetsOverview } from "./bindings/PresetsOverview"
import type { QueueItem } from "./bindings/QueueItem"
import type { SupportedSites } from "./bindings/SupportedSites"
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
    post: null,
    files: null,
  },
  lastDownload: {
    id: 1,
    preset: "Best Video",
    filePath: String.raw`D:\Videos\Me at the zoo [jNQXAC9IVRw].mkv`,
    fileSize: 791_201,
    format: "240p · av01 + opus",
    downloadedAt: 1_793_876_473,
    fileExists: true,
    fileCount: 1,
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

function historyStandIn(id: number, title: string, preset: string, count: number, fileExists = true): HistoryVideo {
  return {
    id,
    ...previewStandIn.video,
    videoId: `video${id}`,
    title,
    uploader: id % 2 ? "jawed" : "Blender Foundation",
    durationSeconds: 19 + id * 97,
    firstDownloadedAt: 1_793_000_000 + id * 86_400,
    lastDownload: {
      id,
      preset,
      filePath: ["C:", "Users", "You", "Videos", `${title} [video${id}].mkv`].join("\\"),
      fileSize: 1_000_000 * id * 7,
      format: "1080p · avc1 + mp4a",
      downloadedAt: 1_793_000_000 + id * 86_400,
      fileExists,
      fileCount: 1,
    },
    downloadCount: count,
  }
}

const postHistoryStandIn: HistoryVideo = {
  ...historyStandIn(4, "A carousel of photos and clips from this week", "", 1),
  site: "instagram",
  siteName: "instagram.com",
  uploader: "Instagram",
  durationSeconds: null,
  bestHeight: null,
  post: { images: 2, videos: 1 },
  lastDownload: {
    id: 4,
    preset: "",
    filePath: String.raw`C:\Users\You\Pictures\Instagram\3780_1.jpg`,
    fileSize: 9_400_000,
    format: null,
    downloadedAt: 1_793_400_000,
    fileExists: true,
    fileCount: 3,
  },
}

const downloadFileStandIns: HistoryFile[] = ["3780_1.jpg", "3780_2.mp4", "3780_3.jpg"].map((name, position) => ({
  position,
  path: ["C:", "Users", "You", "Pictures", "Instagram", name].join("\\"),
  size: 1_200_000 * (position + 1),
  exists: position !== 2,
}))

const historyStandIns: HistoryVideo[] = [
  postHistoryStandIn,
  historyStandIn(3, "Big Buck Bunny 60fps 4K", "1080p MP4", 2),
  historyStandIn(2, "Me at the zoo", "Best Video", 1),
  historyStandIn(1, "A Song Saved as MP3", "Audio MP3", 1, false),
]

const pastDownloadStandIns: PastDownload[] = [
  historyStandIns[0].lastDownload,
  { ...historyStandIns[0].lastDownload, id: 9, preset: "Best Audio", format: "opus", downloadedAt: 1_792_800_000 },
]

const postStandIn: VideoPreview = {
  video: {
    site: "instagram",
    siteName: "instagram.com",
    videoId: "DfPQchkG1tg",
    url: "https://www.instagram.com/p/DfPQchkG1tg/",
    title: "A carousel of photos and clips from this week",
    uploader: "Instagram",
    durationSeconds: null,
    uploadDate: "2026-11-02",
    thumbnailUrl: "https://i.ytimg.com/vi/jNQXAC9IVRw/hqdefault.jpg",
    bestHeight: null,
    post: { images: 2, videos: 4 },
    files: null,
  },
  lastDownload: null,
  inQueue: false,
  missingJsRuntime: false,
}

const sharedFolderStandIn: VideoPreview = {
  video: {
    site: "mega",
    siteName: "mega.nz",
    videoId: "AbCdEfGh",
    url: "https://mega.nz/folder/AbCdEfGh#key",
    title: "Wallpapers 2026",
    uploader: null,
    durationSeconds: null,
    uploadDate: "2026-10-30",
    thumbnailUrl: null,
    bestHeight: null,
    post: null,
    files: { count: 14, bytes: 1_288_490_189 },
  },
  lastDownload: null,
  inQueue: false,
  missingJsRuntime: false,
}

function pendingStandIn(id: number, link: string, fields: Partial<PendingLink> = {}): PendingLink {
  return {
    id,
    link,
    state: "ready",
    preview: null,
    error: null,
    presetId: null,
    startWhenRead: false,
    addedAt: 1_793_876_473,
    caught: false,
    ...fields,
  }
}

/** What api.ts returns when the UI runs in a plain browser, to check layouts without the app. */
export const standIns = {
  appInfo: {
    version: "browser",
    hasMicaBackdrop: false,
    dataFolder: String.raw`C:\Users\You\AppData\Local\io.github.fuzen777.tsundoku`,
    startedInTray: false,
    portable: false,
  } satisfies AppInfo,
  firstRunOptions: (portable: boolean): FirstRunOptions => {
    const dataFolder = portable ? String.raw`E:\Tsundoku\data` : String.raw`C:\Users\You\AppData\Local\io.github.fuzen777.tsundoku`
    return {
      needed: true,
      toolsFolder: String.raw`${dataFolder}\tools`,
      cacheFolder: String.raw`${dataFolder}\cache`,
      defaultDownloadFolder: String.raw`C:\Users\You\Downloads\Tsundoku`,
      besideAppFolder: portable ? String.raw`E:\Tsundoku\Downloads` : null,
      installedToolsFolder: portable ? String.raw`C:\Users\You\AppData\Local\io.github.fuzen777.tsundoku\tools` : null,
      installedToolsFound: portable,
      chosen: null,
    }
  },
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
      { id: "galleryDl", setup: null, path: null, version: null, problem: null },
    ],
    ytDlpChannel: "stable",
    caches: "dataFolder",
    cacheFolder: String.raw`C:\Users\You\AppData\Local\io.github.fuzen777.tsundoku\cache`,
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
    { id: "galleryDl", found: null },
  ] satisfies DetectedTool[],
  toolDownloads: [
    { id: "ytDlp", size: 23_575_744 },
    { id: "ffmpeg", size: 90_993_957 },
    { id: "deno", size: 42_917_472 },
    { id: "galleryDl", size: 22_649_344 },
  ] satisfies ToolDownload[],
  presetsOverview: {
    presets: presetStandIns,
    defaultId: "best-video",
    builtInIds: presetStandIns.map((preset) => preset.id),
    defaultFolder: String.raw`C:\Users\You\Downloads\Tsundoku`,
  } satisfies PresetsOverview,
  presetArguments: ["-f", "bv*+ba/b", "--merge-output-format", "mkv", "-o", "%(title).150B [%(id)s].%(ext)s"],
  /** The history, searched by title only. */
  searchHistory: (query: HistoryQuery): HistoryVideo[] =>
    historyStandIns.filter((video) => video.title.toLowerCase().includes((query.search ?? "").toLowerCase())),
  historyFilters: {
    sites: [{ site: "youtube", siteName: "youtube.com" }],
    presets: ["1080p MP4", "Audio MP3", "Best Audio", "Best Video"],
  } satisfies HistoryFilters,
  videoDownloads: pastDownloadStandIns,
  downloadFiles: downloadFileStandIns,
  supportedSites: {
    sites: [
      { name: "abc.net.au", signedIn: null },
      { name: "Bilibili", signedIn: [] },
      { name: "danbooru", signedIn: null },
      { name: "Instagram", signedIn: [{ browser: "zen", profile: "Main" }] },
      { name: "SoundCloud", signedIn: null },
      { name: "Vimeo", signedIn: null },
      { name: "X", signedIn: [{ browser: "zen", profile: "Main" }, { browser: "vivaldi", profile: "Work" }] },
      { name: "YouTube", signedIn: [] },
    ],
    imagesSetUp: false,
  } satisfies SupportedSites,
  downloadOptions: {
    folder: null,
    atOnce: 2,
    rateLimit: null,
    cookiesBrowser: "automatic",
    postPause: { min: 5, max: 10 },
  } satisfies DownloadSettings,
  addingOptions: {
    watchClipboard: true,
    downloadRightAway: false,
    clipboardShortcut: "ctrlAltD",
    notify: true,
  } satisfies AddingSettings,
  backgroundOptions: { keepInTray: true, startWithWindows: false } satisfies BackgroundSettings,
  defaultDownloadFolder: String.raw`C:\Users\You\Downloads\Tsundoku`,
  folderRules: [
    {
      id: "rule-1",
      enabled: true,
      conditions: [
        { field: "site", operator: "is", value: "Instagram" },
        { field: "uploader", operator: "is", value: "natgeo" },
      ],
      folder: String.raw`D:\Hydrus\Import\{site}\{uploader}`,
    },
    {
      id: "posts",
      enabled: true,
      conditions: [{ field: "kind", operator: "is", value: "post" }],
      folder: "{site}",
    },
    {
      id: "rule-2",
      enabled: false,
      conditions: [
        { field: "kind", operator: "is", value: "audio" },
        { field: "title", operator: "contains", value: "podcast" },
      ],
      folder: "Podcasts",
    },
  ] satisfies FolderRule[],
  destination: {
    folder: String.raw`C:\Users\You\Downloads\Tsundoku\Instagram`,
    source: { kind: "rule", number: 2 },
  } satisfies Destination,
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
  pendingLinks: [
    pendingStandIn(1, "https://www.instagram.com/p/DfPQchkG1tg/", { preview: postStandIn }),
    pendingStandIn(2, "https://www.youtube.com/watch?v=dQw4w9WgXcQ", {
      preview: {
        ...previewStandIn,
        video: { ...previewStandIn.video, videoId: "dQw4w9WgXcQ", title: "A Video Not Downloaded Yet" },
        lastDownload: null,
        missingJsRuntime: false,
      },
      presetId: "audio-mp3",
    }),
    pendingStandIn(3, "https://www.youtube.com/watch?v=jNQXAC9IVRw", { preview: previewStandIn }),
    pendingStandIn(7, "https://mega.nz/folder/AbCdEfGh#key", { preview: sharedFolderStandIn }),
    pendingStandIn(4, "https://www.instagram.com/p/Another1/", { state: "reading" }),
    pendingStandIn(5, "https://www.instagram.com/p/Another2/", { state: "waiting", startWhenRead: true }),
    pendingStandIn(6, "https://www.instagram.com/p/Private1/", {
      state: "failed",
      error: { kind: "signInRequired", detail: "login required", site: "Instagram", cookies: "notSignedIn" },
    }),
    pendingStandIn(8, "https://www.instagram.com/p/Copied1/", { state: "reading", caught: true }),
    pendingStandIn(9, "https://github.com/Fuzen777/Tsundoku", { state: "waiting", caught: true }),
  ] satisfies PendingLink[],
  /** Words that start with http:// or https://, each once: the core also takes links without them. */
  linksInText(text: string): string[] {
    const links = text.split(/\s+/).filter((word) => /^https?:\/\//i.test(word))
    return [...new Set(links)]
  },
  /**
   * A downloaded video after a short wait; a post of images for an Instagram link; a shared folder for a MEGA link;
   * or a failure for a link containing "fail", or "images" (gallery-dl not set up).
   */
  fetchPreview(link: string): Promise<VideoPreview> {
    return new Promise((resolve, reject) =>
      setTimeout(() => {
        if (link.includes("fail")) {
          reject({ kind: "unavailable", detail: "[youtube] aaaaaaaaaaa: This video is unavailable" })
        } else if (link.includes("images")) {
          reject({ kind: "imagesNotSetUp" })
        } else if (link.includes("instagram")) {
          resolve(postStandIn)
        } else if (link.includes("mega.nz")) {
          resolve(sharedFolderStandIn)
        } else {
          resolve(previewStandIn)
        }
      }, 800),
    )
  },
}

import type { AddingSettings } from "./bindings/AddingSettings"
import type { AppInfo } from "./bindings/AppInfo"
import type { AppearanceSettings } from "./bindings/AppearanceSettings"
import type { BackgroundSettings } from "./bindings/BackgroundSettings"
import type { ChangelogSection } from "./bindings/ChangelogSection"
import type { Destination } from "./bindings/Destination"
import type { DetectedTool } from "./bindings/DetectedTool"
import type { DownloadLog } from "./bindings/DownloadLog"
import type { DownloadSettings } from "./bindings/DownloadSettings"
import type { FirstRunOptions } from "./bindings/FirstRunOptions"
import type { FolderRule } from "./bindings/FolderRule"
import type { HistoryFile } from "./bindings/HistoryFile"
import type { HistoryFilters } from "./bindings/HistoryFilters"
import type { HistoryPage } from "./bindings/HistoryPage"
import type { HistoryQuery } from "./bindings/HistoryQuery"
import type { HistoryVideo } from "./bindings/HistoryVideo"
import type { LinkFileError } from "./bindings/LinkFileError"
import type { LinkPreview } from "./bindings/LinkPreview"
import type { ListArrangements } from "./bindings/ListArrangements"
import type { ListedItem } from "./bindings/ListedItem"
import type { Listing } from "./bindings/Listing"
import type { ListingKind } from "./bindings/ListingKind"
import type { PastDownload } from "./bindings/PastDownload"
import type { PendingLink } from "./bindings/PendingLink"
import type { Preset } from "./bindings/Preset"
import type { PresetsOverview } from "./bindings/PresetsOverview"
import type { QueueItem } from "./bindings/QueueItem"
import type { RightAwayRule } from "./bindings/RightAwayRule"
import type { RulePreset } from "./bindings/RulePreset"
import type { SupportedSites } from "./bindings/SupportedSites"
import type { ToolDownload } from "./bindings/ToolDownload"
import type { ToolsOverview } from "./bindings/ToolsOverview"
import type { VideoInfo } from "./bindings/VideoInfo"
import type { WatchedItem } from "./bindings/WatchedItem"
import type { WatchedSource } from "./bindings/WatchedSource"
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
    folder: id % 2 ? String.raw`C:\Users\You\Videos` : String.raw`C:\Users\You\Downloads\Tsundoku\youtube.com`,
    folderChosen: false,
    state,
    overwrite: false,
    addedAt: 1_793_876_473 + id * 60,
    result: null,
    error: null,
    progress: null,
    hold: null,
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
    thumbnailFile: null,
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

/** A long history, to check how the view copes: a third of it posts, one in ten with its file missing. */
function manyHistoryStandIns(count: number): HistoryVideo[] {
  return Array.from({ length: count }, (_, index) => {
    const id = index + 100
    const video = historyStandIn(id, `Video number ${id}`, "Best Video", index % 10 === 1 ? 2 : 1, index % 10 !== 7)
    return index % 3 === 0
      ? { ...postHistoryStandIn, id, videoId: `post${id}`, title: `Post number ${id}`, lastDownload: video.lastDownload }
      : video
  })
}

/** The stand-in history the query matches, by the title, whether it's a post (any other kind is a video), and its
 * file being missing. */
function matchingHistory(query: HistoryQuery, count: number | null): HistoryVideo[] {
  return (count === null ? historyStandIns : manyHistoryStandIns(count)).filter(
    (video) =>
      video.title.toLowerCase().includes((query.search ?? "").toLowerCase()) &&
      (query.kind === null || (query.kind === "post") === (video.post !== null)) &&
      (query.files === null || (query.files === "missing") !== video.lastDownload.fileExists),
  )
}

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
    postFiles: [
      "jNQXAC9IVRw",
      "dQw4w9WgXcQ",
      "aqz-KE-bpKQ",
      null,
      "M7lc1UVf-VE",
      "9bZkp7q19f0",
    ].map((id, index) => ({
      number: index + 1,
      video: index > 0 && index < 5,
      thumbnailUrl: id && `https://i.ytimg.com/vi/${id}/hqdefault.jpg`,
      picked: true,
    })),
  },
  lastDownload: null,
  inQueue: false,
  missingJsRuntime: false,
}

/** The carousel again, downloaded before with two of its files left out (`leftOutFiles`). */
const postAgainStandIn: VideoPreview = {
  ...postStandIn,
  video: { ...postStandIn.video, videoId: "DfPQchkAgain", url: "https://www.instagram.com/p/DfPQchkAgain/" },
  lastDownload: { ...historyStandIns[0].lastDownload, preset: "", format: null, fileCount: 4 },
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
    folder: null,
    startWhenRead: false,
    addedAt: 1_793_876_473 + id * 60,
    caught: false,
    fromFile: false,
    hold: null,
    ...fields,
  }
}

/** What api.ts returns when the UI runs in a plain browser, to check layouts without the app. */
export const standIns = {
  /** A newer release, with `?update` in the address. */
  appUpdate: {
    version: "0.2.0",
    notes: [
      "### Added",
      "",
      "- **Group and sort the queue**: a toolbar above the list groups downloads by folder, site or status.",
      "- **Back up and restore**: Settings → About's `Back Up…` saves your settings and history in one file.",
      "",
      "### Fixed",
      "",
      "- **Long titles** no longer push the row's buttons out of the card. See the [README](https://github.com/Fuzen777/Tsundoku#readme).",
    ].join("\n"),
  },
  /** What changed since the last run, with `?whatsnew` in the address. */
  whatsNew: [
    {
      version: "0.1.47",
      date: "2026-11-06",
      body: "### Added\n\n- **What's New after an update**: the first start on a newer version says what changed, once.",
    },
    {
      version: "0.1.46",
      date: "2026-11-06",
      body: "### Fixed\n\n- **Copy Details' labels**: the button says its name when you hover it, and `Copied` once it has copied.",
    },
  ] satisfies ChangelogSection[],
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
    removedBuiltIns: [],
    defaultFolder: String.raw`C:\Users\You\Downloads\Tsundoku`,
  } satisfies PresetsOverview,
  downloadLog: {
    text: [
      "[youtube] Extracting URL: https://www.youtube.com/watch?v=jNQXAC9IVRw",
      "[download] Destination: Me at the zoo [jNQXAC9IVRw].mp4",
      "[download]  42.0% of 1.20MiB at 1.10MiB/s ETA 00:01",
    ].join("\n"),
    cut: false,
  } satisfies DownloadLog,
  presetArguments: ["-f", "bv*+ba/b", "--merge-output-format", "mkv", "-o", "%(title).150B [%(id)s].%(ext)s"],
  /** A page of the history, searched by title, posts or not, and missing files: a few videos, or `count` of them. */
  historyPage: (query: HistoryQuery, offset: number, limit: number, count: number | null): HistoryPage => {
    const found = matchingHistory(query, count)
    return { videos: found.slice(offset, offset + limit), total: found.length }
  },
  historyIds: (query: HistoryQuery, count: number | null): number[] =>
    matchingHistory(query, count).map((video) => video.id),
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
    postFileName: "{account} {date} {post} {number}",
    postPause: { min: 5, max: 10 },
    sitePauses: { x: { min: 2, max: 5 }, pixiv: null, tumblr: null },
    hourlyFileLimit: null,
  } satisfies DownloadSettings,
  addingOptions: {
    watchClipboard: true,
    downloadRightAway: false,
    clipboardShortcut: "ctrlAltD",
    notify: true,
    notifyFailures: true,
    notifyFinished: true,
  } satisfies AddingSettings,
  backgroundOptions: { keepInTray: true, startWithWindows: false } satisfies BackgroundSettings,
  appearanceOptions: { theme: "auto", accent: "indigo", customAccent: "#165e83" } satisfies AppearanceSettings,
  defaultDownloadFolder: String.raw`C:\Users\You\Downloads\Tsundoku`,
  folderRules: [
    {
      id: "catch-all",
      name: "",
      enabled: true,
      conditions: [],
      folder: "{site}",
      fileName: "",
      presetId: null,
      downloadRightAway: null,
    },
    {
      id: "rule-1",
      name: "Nat Geo",
      enabled: true,
      conditions: [
        { field: "site", operator: "is", value: "Instagram", caseSensitive: false, join: "all" },
        { field: "uploader", operator: "is", value: "natgeo", caseSensitive: false, join: "all" },
      ],
      folder: String.raw`D:\Hydrus\Import\{site}\{account}`,
      fileName: "{account} {post} {number}",
      presetId: null,
      downloadRightAway: null,
    },
    {
      id: "rule-2",
      name: "",
      enabled: false,
      conditions: [
        { field: "kind", operator: "is", value: "audio", caseSensitive: false, join: "all" },
        { field: "title", operator: "contains", value: "podcast", caseSensitive: false, join: "all" },
      ],
      folder: "Podcasts",
      fileName: "",
      presetId: null,
      downloadRightAway: null,
    },
    {
      id: "rule-3",
      name: "",
      enabled: true,
      conditions: [{ field: "title", operator: "contains", value: "zoo", caseSensitive: false, join: "all" }],
      folder: "",
      fileName: "",
      presetId: "best-audio",
      downloadRightAway: true,
    },
  ] satisfies FolderRule[],
  /** The first rule's folder for the video's site, as a new install has it. */
  destination(video: VideoInfo): Destination {
    return {
      folder: ["C:", "Users", "You", "Downloads", "Tsundoku", video.siteName].join("\\"),
      source: { kind: "rule", number: 1 },
    }
  },
  /** Rule 3's preset for a title with "zoo" in it, as the stand-in rules have it. */
  watchedSources: watchedStandIns,
  ruleRightAway(video: VideoInfo): RightAwayRule | null {
    // As the stand-in rules have it: rule 3 downloads videos with "zoo" in their title right away.
    return video.title.toLowerCase().includes("zoo") ? { downloadRightAway: true, rule: 4 } : null
  },
  rulePreset(video: VideoInfo): RulePreset | null {
    if (video.post || video.files || !video.title.toLowerCase().includes("zoo")) return null
    return { presetId: "best-audio", rule: 4 }
  },
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
    queueStandIn(3, "waiting", "Next in Line", {
      hold: { kind: "slowDown", site: "YouTube", until: Date.now() + 12 * 60_000 },
    }),
    queueStandIn(7, "waiting", "Lost Its Connection", {
      hold: { kind: "retry", attempt: 2, attempts: 3, until: Date.now() + 90_000 },
    }),
    queueStandIn(8, "waiting", "Too Big for the Drive", {
      hold: {
        kind: "notEnoughRoom",
        drive: "D:",
        free: 3_328_599_654,
        needed: 15_032_385_536,
        others: 0,
        until: Date.now() + 30_000,
      },
    }),
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
  /** The carousel downloaded before (`postAgainStandIn`) left its second and fifth files out. */
  leftOutFiles(video: VideoInfo): number[] | null {
    return video.videoId === postAgainStandIn.video.videoId ? [2, 5] : null
  },
  pendingLinks: [
    pendingStandIn(1, "https://www.instagram.com/p/DfPQchkG1tg/", {
      preview: {
        ...postStandIn,
        video: {
          ...postStandIn.video,
          postFiles: postStandIn.video.postFiles?.map((file) => ({ ...file, picked: file.number !== 3 })),
        },
      },
    }),
    pendingStandIn(8, "https://www.instagram.com/p/DfPQchkAgain/", {
      preview: {
        ...postAgainStandIn,
        video: {
          ...postAgainStandIn.video,
          postFiles: postAgainStandIn.video.postFiles?.map((file) => ({ ...file, picked: [2, 5].includes(file.number) })),
        },
      },
    }),
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
    pendingStandIn(5, "https://www.instagram.com/p/Another2/", {
      state: "waiting",
      startWhenRead: true,
      hold: { site: "Instagram", until: Date.now() + 8 * 60_000 },
    }),
    pendingStandIn(6, "https://www.instagram.com/p/Private1/", {
      state: "failed",
      error: { kind: "signInRequired", detail: "login required", site: "Instagram", cookies: "notSignedIn" },
    }),
    pendingStandIn(8, "https://www.instagram.com/p/Copied1/", { state: "reading", caught: true }),
    pendingStandIn(9, "https://github.com/Fuzen777/Tsundoku", { state: "waiting", caught: true }),
    pendingStandIn(10, "https://www.bbc.com/news/an-article", { state: "waiting", fromFile: true }),
    pendingStandIn(11, "https://vimeo.com/76979871", { state: "reading", fromFile: true }),
  ] satisfies PendingLink[],
  listArrangements: {
    pending: { group: "none", byStatus: false, sort: "added", descending: false },
    queue: { group: "none", byStatus: false, sort: "added", descending: false },
  } satisfies ListArrangements,
  /** Words that start with http:// or https://, each once: the core also takes links without them. */
  linksInText(text: string): string[] {
    const links = text.split(/\s+/).filter((word) => /^https?:\/\//i.test(word))
    return [...new Set(links)]
  },
  /** As the core checks a dropped file: by its extension, and at most 10 MB. */
  checkLinkFile(name: string, size: number): void {
    if (!/\.(txt|html?|url)$/i.test(name)) throw { kind: "unsupportedType", name } satisfies LinkFileError
    const maxBytes = 10 * 1024 * 1024
    if (size > maxBytes) throw { kind: "tooLarge", name, maxBytes } satisfies LinkFileError
  },
  /** Every http or https link in the file's text, each once, or noLinks. */
  linksInLinkFile(name: string, bytes: Uint8Array): string[] {
    const links = new TextDecoder().decode(bytes).match(/https?:\/\/[^\s"'<>]+/gi) ?? []
    if (links.length === 0) throw { kind: "noLinks", name } satisfies LinkFileError
    return [...new Set(links)]
  },
  /**
   * A downloaded video after a short wait; a post of images for an Instagram link; a shared folder for a MEGA link; a
   * listing for a channel, playlist or account (`listingKind`); or a failure for a link containing "fail", or "images"
   * (gallery-dl not set up).
   */
  fetchPreview(link: string): Promise<LinkPreview> {
    const kind = standIns.listingKind(link)
    return new Promise((resolve, reject) =>
      setTimeout(() => {
        if (link.includes("fail")) {
          reject({ kind: "unavailable", detail: "[youtube] aaaaaaaaaaa: This video is unavailable" })
        } else if (link.includes("images")) {
          reject({ kind: "imagesNotSetUp" })
        } else if (kind) {
          resolve({ kind: "listing", listing: listingStandIn(kind) })
        } else if (link.includes("instagram")) {
          resolve({ kind: "video", ...postStandIn })
        } else if (link.includes("mega.nz")) {
          resolve({ kind: "video", ...sharedFolderStandIn })
        } else {
          resolve({ kind: "video", ...previewStandIn })
        }
      }, kind ? 1600 : 800),
    )
  },
  /** As the core tells them: a YouTube channel (`/@name`) or playlist (`/playlist?list=`), an Instagram account. */
  listingKind(link: string): ListingKind | null {
    if (/youtube\.com\/(@[^/?#]+|channel\/|c\/|user\/)/.test(link)) return "channel"
    if (/youtube\.com\/playlist\?list=/.test(link)) return "playlist"
    if (/instagram\.com\/(?!p\/|reels?\/|stories\/|explore\/)[\w.]+\/?(posts\/?|reels\/?)?$/.test(link)) return "account"
    return null
  },
}

/** A listing with a downloaded item and a queued one: 60 for a channel or an account, 300 for a playlist, to check a
 * long list. */
/** Two watched sources: a channel with new and skipped videos, and an account whose last check failed. */
function watchedStandIns(): WatchedSource[] {
  const toItem = (item: ListedItem, index: number): WatchedItem => ({
    ...item,
    firstSeenAt: Math.floor(Date.now() / 1000) - index * 3600,
  })
  const channel = listingStandIn("channel")
  const account = listingStandIn("account")
  const now = Math.floor(Date.now() / 1000)
  return [
    {
      id: 1,
      url: channel.url,
      kind: "channel",
      name: channel.title,
      siteName: channel.siteName,
      thumbnailUrl: channel.thumbnailUrl,
      folder: "D:\\Videos\\{uploader}",
      presetId: "best-audio",
      addedAt: now - 7 * 86_400,
      lastCheckedAt: now - 2 * 3600,
      lastError: null,
      checking: false,
      newItems: channel.items.slice(0, 3).map(toItem),
      skippedItems: channel.items.slice(5, 7).map(toItem),
    },
    {
      id: 2,
      url: "https://www.instagram.com/someaccount/posts/",
      kind: "account",
      name: account.title,
      siteName: account.siteName,
      thumbnailUrl: account.thumbnailUrl,
      folder: null,
      presetId: null,
      addedAt: now - 2 * 86_400,
      lastCheckedAt: now - 600,
      lastError: { kind: "signInRequired", detail: "login required", site: "Instagram", cookies: "notSignedIn" },
      checking: false,
      newItems: [],
      skippedItems: [],
    },
  ]
}

function listingStandIn(kind: ListingKind): Listing {
  const posts = kind === "account"
  const count = kind === "playlist" ? 300 : 60
  const daysAgo = (days: number) => new Date(Date.now() - days * 86_400_000).toISOString().slice(0, 10)
  const items: ListedItem[] = Array.from({ length: count }, (_, index) => {
    const base = posts ? postStandIn.video : previewStandIn.video
    return {
      video: {
        ...base,
        videoId: `listed${index}`,
        url: `${base.url}#${index}`,
        title: posts ? `Post number ${index + 1}, with a caption long enough to need cutting short` : `Video number ${index + 1}`,
        durationSeconds: posts ? null : 45 + index * 97,
        uploadDate: posts ? daysAgo(index * 3) : null,
      },
      approximateDate: posts ? null : daysAgo(index * 5),
      lastDownload: index === 2 ? previewStandIn.lastDownload : null,
      inQueue: index === 4,
    }
  })
  return {
    kind,
    title: { channel: "jawed", playlist: "A Long Playlist", account: "Some Account" }[kind],
    uploader: kind === "playlist" ? "jawed" : null,
    siteName: posts ? "instagram.com" : "youtube.com",
    url: "https://www.youtube.com/@jawed/videos",
    thumbnailUrl: previewStandIn.video.thumbnailUrl,
    total: posts ? 8634 : null,
    newestOnly: kind !== "playlist",
    items,
  }
}

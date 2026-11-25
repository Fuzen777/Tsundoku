import type { FanboxSignIn } from "./bindings/FanboxSignIn"
import type { PixivAccount } from "./bindings/PixivAccount"
import type { PixivWarningStatus } from "./bindings/PixivWarningStatus"
import type { AddingSettings } from "./bindings/AddingSettings"
import type { AppInfo } from "./bindings/AppInfo"
import type { AppearanceSettings } from "./bindings/AppearanceSettings"
import type { BackgroundSettings } from "./bindings/BackgroundSettings"
import type { ChangelogSection } from "./bindings/ChangelogSection"
import type { ClickNLoadStatus } from "./bindings/ClickNLoadStatus"
import type { Destination } from "./bindings/Destination"
import type { DetectedTool } from "./bindings/DetectedTool"
import type { DownloadLog } from "./bindings/DownloadLog"
import type { DownloadSettings } from "./bindings/DownloadSettings"
import type { FirstRunOptions } from "./bindings/FirstRunOptions"
import type { FolderRule } from "./bindings/FolderRule"
import type { HistoryFile } from "./bindings/HistoryFile"
import type { HistoryData } from "./bindings/HistoryData"
import type { HistoryFilters } from "./bindings/HistoryFilters"
import type { HistoryGroupCount } from "./bindings/HistoryGroupCount"
import type { HistoryPage } from "./bindings/HistoryPage"
import type { HistoryQuery } from "./bindings/HistoryQuery"
import type { HistorySize } from "./bindings/HistorySize"
import type { HistoryVideo } from "./bindings/HistoryVideo"
import type { LinkFileError } from "./bindings/LinkFileError"
import type { LinkPreview } from "./bindings/LinkPreview"
import type { ListArrangements } from "./bindings/ListArrangements"
import type { PickerLayouts } from "./bindings/PickerLayouts"
import type { ListedItem } from "./bindings/ListedItem"
import type { Listing } from "./bindings/Listing"
import type { ListingKind } from "./bindings/ListingKind"
import type { ListingMore } from "./bindings/ListingMore"
import type { MoreItems } from "./bindings/MoreItems"
import type { MoreRead } from "./bindings/MoreRead"
import type { PastDownload } from "./bindings/PastDownload"
import type { PendingLink } from "./bindings/PendingLink"
import type { Preset } from "./bindings/Preset"
import type { PresetsOverview } from "./bindings/PresetsOverview"
import type { QueueItem } from "./bindings/QueueItem"
import type { QueueSource } from "./bindings/QueueSource"
import type { RightAwayRule } from "./bindings/RightAwayRule"
import type { RulePreset } from "./bindings/RulePreset"
import type { SharedFile } from "./bindings/SharedFile"
import type { SupportedSites } from "./bindings/SupportedSites"
import type { ToolDownload } from "./bindings/ToolDownload"
import type { ToolsOverview } from "./bindings/ToolsOverview"
import type { UnpackingRule } from "./bindings/UnpackingRule"
import type { VideoInfo } from "./bindings/VideoInfo"
import type { WatchedItem } from "./bindings/WatchedItem"
import type { WatchedSource } from "./bindings/WatchedSource"
import type { WatchedSettings } from "./bindings/WatchedSettings"
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

/** A channel and an account some stand-in downloads were picked from. */
const channelSourceStandIn: QueueSource = {
  url: "https://www.youtube.com/@jawed/videos",
  kind: "channel",
  name: "jawed",
  siteName: "youtube.com",
  thumbnailUrl: "https://i.ytimg.com/vi/jNQXAC9IVRw/hqdefault.jpg",
}
const accountSourceStandIn: QueueSource = {
  url: "https://www.instagram.com/someaccount/",
  kind: "account",
  name: "Some Account",
  siteName: "instagram.com",
  thumbnailUrl: null,
}

/** A stand-in download's result, its file named after it. */
function finishedStandIn(id: number): QueueItem["result"] {
  return { filePath: String.raw`C:\Users\You\Videos\video${id}.mkv`, fileSize: 8_000_000 + id * 1_000_000, downloadId: id }
}

/** What a stand-in post's download starts with: the post, no preset. */
function postQueueStandIn(id: number, title: string): Partial<QueueItem> {
  return {
    video: { ...postStandIn.video, videoId: `post${id}`, title },
    preset: null,
  }
}

function queueStandIn(id: number, state: QueueItem["state"], title: string, extra: Partial<QueueItem> = {}): QueueItem {
  return {
    id,
    video: { ...previewStandIn.video, videoId: `video${id}`, title },
    preset: presetStandIns[id % presetStandIns.length],
    folder: id % 2 ? String.raw`C:\Users\You\Videos` : String.raw`C:\Users\You\Downloads\Tsundoku\youtube.com`,
    folderChosen: false,
    ruleId: null,
    // The catch-all's {site} folder, or the download folder; a rule deleted since for a few, and one added before what
    // gave its folder was kept.
    folderFrom:
      id % 5 === 0
        ? { kind: "rule", id: "rule-deleted" }
        : id === 4
          ? null
          : id % 2
            ? { kind: "downloadFolder" }
            : { kind: "rule", id: "catch-all" },
    source: null,
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
  post: { images: 2, videos: 1, others: 0 },
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
  const videos =
    query.watchedSource !== null
      ? watchedHistoryStandIns(query.watchedSource)
      : count === null
        ? historyStandIns
        : manyHistoryStandIns(count)
  const found = videos.filter(
    (video) =>
      video.title.toLowerCase().includes((query.search ?? "").toLowerCase()) &&
      (query.kind === null || (query.kind === "post") === (video.post !== null)) &&
      (query.files === null || (query.files === "missing") !== video.lastDownload.fileExists),
  )
  // Grouped, each group's videos come together, as the core gives them.
  if (query.group === "none") return found
  const order = new Map(historyGroupsOf(query, found).map((group, index) => [group.key, index]))
  return found
    .map((video, index) => ({ video, index, group: order.get(standInGroup(query, video).key) ?? 0 }))
    .sort((first, second) => first.group - second.group || first.index - second.index)
    .map(({ video }) => video)
}

/** A stand-in video's group, roughly as the core works it out. */
function standInGroup(query: HistoryQuery, video: HistoryVideo): { key: string | null; label: string | null } {
  const folder = video.lastDownload.filePath.replace(/[\\/][^\\/]*$/, "")
  const month = new Date(video.lastDownload.downloadedAt * 1000).toISOString().slice(0, 7)
  switch (query.group) {
    case "site":
      return { key: video.site, label: video.siteName }
    case "destination":
      return { key: folder.toLowerCase(), label: folder }
    case "uploader":
      return { key: video.uploader?.toLowerCase() || null, label: video.uploader }
    case "month":
      return { key: month, label: month }
    case "none":
      return { key: null, label: null }
  }
}

/** The stand-in videos' groups, by key, months newest first and the rest by name, the videos without one last. */
function historyGroupsOf(query: HistoryQuery, videos: readonly HistoryVideo[]): HistoryGroupCount[] {
  const groups = new Map<string | null, HistoryGroupCount>()
  for (const video of videos) {
    const { key, label } = standInGroup(query, video)
    const group = groups.get(key)
    if (group) group.count += 1
    else groups.set(key, { key, label, count: 1 })
  }
  const sorted = [...groups.values()].sort((first, second) =>
    first.key === null || second.key === null
      ? Number(first.key === null) - Number(second.key === null)
      : (first.label ?? "").localeCompare(second.label ?? ""),
  )
  return query.group === "month" ? sorted.reverse() : sorted
}

/** A watched source's downloads: as many as its card counts, from its stand-in listing's items (an account's posts,
 * a channel's videos), the latest downloaded first, the fifth one's file gone. */
function watchedHistoryStandIns(sourceId: number): HistoryVideo[] {
  const source = watchedStandIns().find((watched) => watched.id === sourceId)
  if (!source) return []
  const listing = listingStandIn(source.kind === "account" && source.siteName === "instagram.com" ? "account" : "channel")
  const now = Math.floor(Date.now() / 1000)
  return listing.items.slice(10 + sourceId, 10 + sourceId + source.downloadedCount).map((item, index) => ({
    ...item.video,
    id: 1000 * sourceId + index,
    firstDownloadedAt: now - (index + 1) * 86_400,
    lastDownload: {
      ...historyStandIns[1].lastDownload,
      id: 1000 * sourceId + index,
      downloadedAt: now - (index + 1) * 86_400,
      fileExists: index !== 4,
    },
    downloadCount: 1,
    thumbnailFile: null,
  }))
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
    post: { images: 2, videos: 4, others: 0 },
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

/** A shared folder's `count` files as a host lists them, its own name first in each path: two at its top, the rest in
 * nested folders, a block each. */
function sharedFolderFiles(count: number): SharedFile[] {
  const folders = ["Desktop/4K", "Desktop/Ultrawide", "Phone", "Extras/Sketches", "Extras/Process Videos"]
  const top = ["Read Me.txt", "Credits.txt"].slice(0, count)
  const perFolder = Math.max(1, Math.ceil((count - top.length) / folders.length))
  const inFolders = Array.from({ length: count - top.length }, (_, index) => {
    const folder = folders[Math.min(folders.length - 1, Math.floor(index / perFolder))]
    const number = String((index % perFolder) + 1).padStart(String(perFolder).length, "0")
    const video = folder.endsWith("Videos")
    return {
      path: `Wallpapers 2026/${folder}/${video ? "Timelapse" : "Wallpaper"} ${number}.${video ? "mp4" : "jpg"}`,
      size: video ? 48_000_000 + index * 1_234_567 : 1_500_000 + ((index * 7919) % 97) * 61_440,
      picked: true,
    }
  })
  return [...top.map((name, index) => ({ path: `Wallpapers 2026/${name}`, size: 2_048 + index * 517, picked: true })), ...inFolders]
}

/** A MEGA folder of `count` files (`?folder=2000` in the address for a big one). */
function sharedFolderStandInOf(count: number): VideoPreview {
  const sharedFiles = sharedFolderFiles(count)
  return {
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
      files: { count: sharedFiles.length, bytes: sharedFiles.reduce((sum, file) => sum + file.size, 0) },
      sharedFiles,
    },
    lastDownload: null,
    inQueue: false,
    missingJsRuntime: false,
  }
}

const sharedFolderStandIn = sharedFolderStandInOf(25)

/** The folder again, downloaded before without five of its files (`leftOutSharedFiles`). */
const sharedFolderAgainStandIn: VideoPreview = {
  ...sharedFolderStandIn,
  video: { ...sharedFolderStandIn.video, videoId: "AgainAgain", url: "https://mega.nz/folder/AgainAgain#key" },
  lastDownload: { ...historyStandIns[0].lastDownload, preset: "", format: null, fileCount: 20 },
}

/** The files the folder downloaded before (`sharedFolderAgainStandIn`) didn't take. */
const sharedFolderLeftOut = [1, 7, 8, 15, 24]

/** A folder on Gofile as gallery-dl reads it: a post of files that aren't all pictures, each with its name and size
 * (the last with neither), so its files show as a list. */
const filePostStandIn: VideoPreview = {
  video: {
    site: "gofile",
    siteName: "gofile.io",
    videoId: "Ab12Cd",
    url: "https://gofile.io/d/Ab12Cd",
    title: "Game Jam Build",
    uploader: null,
    durationSeconds: null,
    uploadDate: "2026-11-01",
    thumbnailUrl: null,
    bestHeight: null,
    post: { images: 1, videos: 1, others: 4 },
    files: null,
    postFiles: (
      [
        ["Game Jam Build (Windows).zip", 734_003_200, false],
        ["Read Me.txt", 2_150, false],
        ["Cover.jpg", 412_000, false],
        ["Trailer.mp4", 96_468_992, true],
        ["Manual.pdf", 5_242_880, false],
        [null, null, false],
      ] as const
    ).map(([name, size, video], index) => ({
      number: index + 1,
      video,
      thumbnailUrl: null,
      picked: true,
      ...(name && { name }),
      ...(size && { size }),
    })),
  },
  lastDownload: null,
  inQueue: false,
  missingJsRuntime: false,
}

/** The Gofile folder again, downloaded before with its second and sixth files left out (`leftOutFiles`). */
const filePostAgainStandIn: VideoPreview = {
  ...filePostStandIn,
  video: { ...filePostStandIn.video, videoId: "AgainAgain", url: "https://gofile.io/d/AgainAgain" },
  lastDownload: { ...historyStandIns[0].lastDownload, preset: "", format: null, fileCount: 4 },
}

/** The folder with only the files at `picked` ticked. */
function withSharedFilesPicked(video: VideoInfo, picked: number[]): VideoInfo {
  return { ...video, sharedFiles: video.sharedFiles?.map((file, index) => ({ ...file, picked: picked.includes(index) })) }
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
    ruleId: null,
    startWhenRead: false,
    addedAt: 1_793_876_473 + id * 60,
    caught: false,
    fromFile: false,
    fromPage: false,
    hold: null,
    listing: null,
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
  /** Every version's changes, for the What's New card opened from Settings → About. */
  releaseNotes: [
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
    {
      version: "0.1.0",
      date: "2026-11-05",
      body: [
        "The first release.",
        "",
        "### Added",
        "",
        ...Array.from(
          { length: 14 },
          (_, index) => `- **Feature ${index + 1}**: what it does, in a sentence long enough to wrap at the window's minimum width.`,
        ),
      ].join("\n"),
    },
    { version: "0.0.9", date: null, body: "- **Before the first release**: a section without a date." },
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
  historyGroups: (query: HistoryQuery, count: number | null): HistoryGroupCount[] =>
    query.group === "none" ? [] : historyGroupsOf(query, matchingHistory(query, count)),
  historyData: {
    database: 12_582_912,
    thumbnails: 30_146_560,
    thumbnailCount: 5000,
    logs: 4_194_304,
    logCount: 120,
    archive: 51_200,
  } satisfies HistoryData,
  /** What the matching stand-ins' latest downloads take, their missing files counted apart. */
  historySize: (query: HistoryQuery, count: number | null): HistorySize =>
    matchingHistory(query, count).reduce(
      (size, { lastDownload }) =>
        lastDownload.fileExists
          ? { ...size, bytes: size.bytes + (lastDownload.fileSize ?? 0), files: size.files + lastDownload.fileCount }
          : { ...size, missing: size.missing + lastDownload.fileCount },
      { bytes: 0, files: 0, missing: 0 },
    ),
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
    siteAtOnce: 0,
    rateLimit: null,
    cookiesBrowser: "automatic",
    postFileName: "{account} {date} {post} {number}",
    pacing: {
      instagram: { atOnce: 1, pause: { min: 3, max: 5 } },
      x: { atOnce: 2, pause: { min: 2, max: 5 } },
      pixiv: { atOnce: 1, pause: { min: 0, max: 2 } },
      tumblr: { atOnce: 1, pause: { min: 0, max: 0 } },
      fanbox: { atOnce: 1, pause: { min: 1, max: 2 } },
    },
    hourlyFileLimit: null,
    unpack: { enabled: true, place: "smart", after: "recycleBin", passwords: [] },
    savePastPasswords: false,
  } satisfies DownloadSettings,
  addingOptions: {
    watchClipboard: true,
    downloadRightAway: false,
    clipboardShortcut: "ctrlAltD",
    notify: true,
    notifyFailures: true,
    notifyFinished: true,
    clickNLoad: false,
    pickInFullPage: true,
  } satisfies AddingSettings,
  clickNLoadStatus: { state: { kind: "off" }, port: 9666, senders: ["FileCrypt"] } satisfies ClickNLoadStatus,
  pixivAccount: { name: "Some One", account: "someone" } satisfies PixivAccount,
  fanboxSignIn: { signedInAt: Date.now() - 86_400_000 } satisfies FanboxSignIn,
  pixivWarningChecked: {
    warning: null,
    checkedAt: Date.now() - 12 * 60_000,
    problem: null,
    checking: false,
  } satisfies PixivWarningStatus,
  pixivWarned: {
    warning: { sentAt: Date.now() - 3 * 3_600_000 },
    checkedAt: Date.now() - 60_000,
    problem: null,
    checking: false,
  } satisfies PixivWarningStatus,
  backgroundOptions: { keepInTray: true, startWithWindows: false } satisfies BackgroundSettings,
  watchedOptions: { newItemsStart: "unticked" } satisfies WatchedSettings,
  appearanceOptions: {
    theme: "auto",
    accent: "indigo",
    customAccent: "#165e83",
    sidebarCollapsed: false,
  } satisfies AppearanceSettings,
  defaultDownloadFolder: String.raw`C:\Users\You\Downloads\Tsundoku`,
  folderRules: [
    {
      id: "catch-all",
      name: "",
      enabled: true,
      manual: false,
      conditions: [],
      folder: "{site}",
      fileName: "",
      missingValues: "standIn",
      presetId: null,
      downloadRightAway: null,
      unpack: null,
    },
    {
      id: "rule-1",
      name: "Nat Geo",
      enabled: true,
      manual: false,
      conditions: [
        { field: "site", operator: "is", value: "Instagram", caseSensitive: false, join: "all" },
        { field: "uploader", operator: "is", value: "natgeo", caseSensitive: false, join: "all" },
      ],
      folder: String.raw`D:\Hydrus\Import\{site}\{account}`,
      fileName: "{account} {post} {number}",
      missingValues: "leaveOut",
      presetId: null,
      downloadRightAway: null,
      unpack: null,
    },
    {
      id: "rule-2",
      name: "",
      enabled: false,
      manual: false,
      conditions: [
        { field: "kind", operator: "is", value: "audio", caseSensitive: false, join: "all" },
        { field: "title", operator: "contains", value: "podcast", caseSensitive: false, join: "all" },
      ],
      folder: "Podcasts",
      fileName: "",
      missingValues: "standIn",
      presetId: null,
      downloadRightAway: null,
      unpack: null,
    },
    {
      id: "rule-3",
      name: "",
      enabled: true,
      manual: false,
      conditions: [{ field: "title", operator: "contains", value: "zoo", caseSensitive: false, join: "all" }],
      folder: "",
      fileName: "",
      missingValues: "standIn",
      presetId: "best-audio",
      downloadRightAway: true,
      unpack: null,
    },
    {
      id: "rule-4",
      name: "MEGA Packs",
      enabled: true,
      manual: false,
      conditions: [{ field: "site", operator: "is", value: "MEGA", caseSensitive: false, join: "all" }],
      folder: "",
      fileName: "",
      missingValues: "standIn",
      presetId: null,
      downloadRightAway: null,
      unpack: "inPlace",
    },
    {
      id: "rule-5",
      name: "Voice Works",
      enabled: true,
      manual: true,
      conditions: [{ field: "title", operator: "matches", value: String.raw`(RJ\d+)`, caseSensitive: false, join: "all" }],
      folder: String.raw`D:\Voice\{1}`,
      fileName: "{1}",
      missingValues: "leaveOut",
      presetId: null,
      downloadRightAway: null,
      unpack: "archiveFolder",
    },
  ] satisfies FolderRule[],
  /** The first rule's folder for the video's site, as a new install has it, or the folder of the rule picked for it. */
  destination(video: VideoInfo, ruleId: string | null = null): Destination {
    const picked = standIns.folderRules.findIndex((rule) => rule.id === ruleId && rule.folder !== "")
    if (picked >= 0) {
      const folder = standIns.folderRules[picked].folder
        .replace("{site}", video.siteName)
        .replace("{account}", "natgeo")
        .replace("{1}", /RJ\d+/i.exec(video.title)?.[0] ?? "Unknown")
      const rule = standIns.folderRules[picked]
      // As the stand-in manual rule has it: its pattern finds an RJ code in the title.
      const pickedMismatch =
        rule.manual && !/RJ\d+/i.test(video.title) ? { rule: picked + 1, nothingCaptured: true } : undefined
      return { folder, source: { kind: "rule", number: picked + 1 }, pickedMismatch }
    }
    return {
      folder: ["C:", "Users", "You", "Downloads", "Tsundoku", video.siteName].join("\\"),
      source: { kind: "rule", number: 1 },
    }
  },
  /** Rule 3's preset for a title with "zoo" in it, as the stand-in rules have it. */
  watchedSources: watchedStandIns,
  /** A watched source's pictures, as the core lists them: its listing's, then its items', a post's files one by one. */
  watchedPictureChoices(id: number): string[] {
    const source = watchedStandIns().find((watched) => watched.id === id)
    if (!source) return []
    const items = [...source.newItems, ...source.skippedItems].flatMap(({ video }) => {
      const files = (video.postFiles ?? []).flatMap((file) => (file.thumbnailUrl ? [file.thumbnailUrl] : []))
      return files.length > 0 ? files : video.thumbnailUrl ? [video.thumbnailUrl] : []
    })
    return [...new Set([source.thumbnailUrl, ...items].filter((url): url is string => url !== null))]
  },
  ruleRightAway(video: VideoInfo): RightAwayRule | null {
    // As the stand-in rules have it: rule 3 downloads videos with "zoo" in their title right away.
    return video.title.toLowerCase().includes("zoo") ? { downloadRightAway: true, rule: 4 } : null
  },
  ruleUnpacking(video: VideoInfo, ruleId: string | null = null): UnpackingRule | null {
    const picked = standIns.folderRules.findIndex((rule) => rule.id === ruleId && rule.unpack !== null)
    const unpacking = standIns.folderRules[picked]?.unpack
    if (unpacking) return { unpacking, rule: picked + 1 }
    // As the stand-in rules have it: rule 5 unpacks MEGA's archives in place.
    return video.site === "mega" ? { unpacking: "inPlace", rule: 5 } : null
  },
  rulePreset(video: VideoInfo, ruleId: string | null = null): RulePreset | null {
    if (video.post || video.files) return null
    const picked = standIns.folderRules.findIndex((rule) => rule.id === ruleId && rule.presetId !== null)
    const presetId = standIns.folderRules[picked]?.presetId
    if (presetId) return { presetId, rule: picked + 1 }
    if (!video.title.toLowerCase().includes("zoo")) return null
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
    queueStandIn(9, "waiting", "Behind a Check", {
      hold: { kind: "browserCheck", site: "MediaFire", files: 1 },
    }),
    queueStandIn(10, "running", "An Album Being Unpacked", {
      progress: {
        stage: "unpacking",
        fraction: 0.63,
        downloadedBytes: 1_352_663_040,
        totalBytes: 2_147_483_648,
        speed: null,
        eta: null,
      },
    }),
    queueStandIn(12, "waiting", "Behind a Password", {
      hold: { kind: "needsPassword", wrong: false, tried: 2 },
    }),
    queueStandIn(13, "waiting", "Wallpapers 2026", {
      video: withSharedFilesPicked(sharedFolderStandIn.video, [0, 3, 4, 5, 9, 22]),
      preset: null,
    }),
    queueStandIn(4, "stopped", "Stopped Halfway"),
    queueStandIn(5, "failed", "A Video That Failed", {
      error: { kind: "ytDlp", error: { kind: "unavailable", detail: "[youtube] video5: This video is unavailable" } },
    }),
    queueStandIn(11, "failed", "An Archive With a Password", {
      error: { kind: "unpack", archive: "Album.part1.rar", problem: { kind: "wrongPassword", tried: 2 } },
    }),
    queueStandIn(6, "done", "Finished Already", {
      result: {
        filePath: String.raw`C:\Users\You\Videos\Finished Already [video6].mkv`,
        fileSize: 52_428_800,
        downloadId: 1,
      },
    }),
    // Picked together from a channel, and from an account, among the others: each shows under its source's heading.
    queueStandIn(20, "done", "Video number 1", { source: channelSourceStandIn, result: finishedStandIn(20) }),
    queueStandIn(21, "running", "Video number 2", {
      source: channelSourceStandIn,
      progress: {
        stage: "downloadingVideo",
        fraction: 0.6,
        downloadedBytes: 60_000_000,
        totalBytes: 100_000_000,
        speed: 3_100_000,
        eta: 13,
      },
    }),
    queueStandIn(22, "waiting", "Video number 4", { source: channelSourceStandIn }),
    queueStandIn(23, "waiting", "Video number 5", { source: channelSourceStandIn }),
    queueStandIn(24, "done", "Post number 1", {
      ...postQueueStandIn(24, "Post number 1"),
      source: accountSourceStandIn,
      result: finishedStandIn(24),
    }),
    queueStandIn(25, "failed", "Post number 2", {
      ...postQueueStandIn(25, "Post number 2"),
      source: accountSourceStandIn,
      error: { kind: "ytDlp", error: { kind: "network", detail: "Connection reset by peer" } },
    }),
    queueStandIn(26, "done", "Post number 3", {
      ...postQueueStandIn(26, "Post number 3"),
      source: accountSourceStandIn,
      result: finishedStandIn(26),
    }),
  ] satisfies QueueItem[],
  /** The carousel downloaded before (`postAgainStandIn`) left its second and fifth files out, the Gofile folder
   * (`filePostAgainStandIn`) its second and sixth. */
  leftOutFiles(video: VideoInfo): number[] | null {
    if (video.videoId === postAgainStandIn.video.videoId) return [2, 5]
    return video.url === filePostAgainStandIn.video.url ? [2, 6] : null
  },
  /** The folder downloaded before (`sharedFolderAgainStandIn`) didn't take five of its files. */
  leftOutSharedFiles(video: VideoInfo): number[] | null {
    return video.videoId === sharedFolderAgainStandIn.video.videoId ? sharedFolderLeftOut : null
  },
  pendingLinks: [
    pendingStandIn(1, "https://www.instagram.com/p/DfPQchkG1tg/", {
      ruleId: "rule-1",
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
    pendingStandIn(7, "https://mega.nz/folder/AbCdEfGh#key", {
      preview: { ...sharedFolderStandIn, video: withSharedFilesPicked(sharedFolderStandIn.video, [0, 2, 3, 4, 5, 6]) },
    }),
    pendingStandIn(13, "https://mega.nz/folder/AgainAgain#key", {
      preview: {
        ...sharedFolderAgainStandIn,
        video: withSharedFilesPicked(sharedFolderAgainStandIn.video, sharedFolderLeftOut),
      },
    }),
    // Read before its first download, so every file is still ticked: the sentence over them doesn't claim they're the
    // ones not picked.
    pendingStandIn(14, "https://gofile.io/d/AgainAgain", { preview: filePostAgainStandIn }),
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
    pendingStandIn(40, "https://someone.fanbox.cc/posts/12926792", {
      state: "failed",
      error: { kind: "fanboxSignInNeeded", refused: false },
    }),
    pendingStandIn(12, "https://mega.nz/#P!AgESNFZ4mrwAAQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eH9VsEHziCLjOu1HNUk6o", {
      state: "needsPassword",
      error: { kind: "passwordNeeded", wrong: true, tried: 0 },
    }),
    pendingStandIn(16, "https://www.youtube.com/@NASA", { state: "needsPicking", caught: true, listing: "channel" }),
    pendingStandIn(17, "https://x.com/NASA", { state: "needsPicking", caught: true, listing: "account" }),
    pendingStandIn(18, "https://soundcloud.com/someone/sets/an-album", { state: "needsPicking" }),
    pendingStandIn(15, "https://www.instagram.com/p/Copied1/", { state: "reading", caught: true }),
    pendingStandIn(9, "https://github.com/Fuzen777/Tsundoku", { state: "waiting", caught: true }),
    pendingStandIn(10, "https://www.bbc.com/news/an-article", { state: "waiting", fromFile: true }),
    pendingStandIn(11, "https://vimeo.com/76979871", { state: "reading", fromFile: true }),
  ] satisfies PendingLink[],
  listArrangements: {
    pending: { group: "none", byStatus: false, sort: "added", descending: false },
    queue: { group: "none", byStatus: false, sort: "added", descending: false },
    watched: { show: "sources", group: "site", sort: "mostNew", descending: true },
    history: { group: "none", sort: "downloaded", descending: true },
  } satisfies ListArrangements,
  pickerLayouts: {
    items: { view: "gallery", size: 2, hideDownloaded: false },
    postFiles: { view: "gallery", size: 1, hideDownloaded: false },
    queueSources: { view: "gallery", size: 2, hideDownloaded: false },
    pending: { view: "list", size: 2, hideDownloaded: false },
    history: { view: "list", size: 2, hideDownloaded: false },
  } satisfies PickerLayouts,
  /** Words that start with http:// or https://, each once: the core also takes links without them. */
  linksInText(text: string): string[] {
    const links = text.split(/\s+/).filter((word) => /^https?:\/\//i.test(word))
    return [...new Set(links)]
  },
  /** The same words, or the text's one word when it has a dot in it, as the core takes "youtu.be/…". */
  linksInCopy(text: string): string[] {
    const words = text.split(/\s+/).filter(Boolean)
    const [only] = words
    if (words.length === 1 && only.includes(".") && !only.includes("://")) return [`https://${only}`]
    return standIns.linksInText(text)
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
   * A downloaded video after a short wait; a post of images for an Instagram link; a post of files for a Gofile link
   * (downloaded before when the link has "again"); a shared folder for a MEGA link (of
   * `folderSize` files, else 25; downloaded before when the link has "again"); a listing for a channel, playlist or
   * account (`listingKind`); or a failure for a link containing "fail", or "images" (gallery-dl not set up). A link
   * with "#P!" needs a password, "secret".
   */
  fetchPreview(link: string, password: string | null = null, folderSize: number | null = null): Promise<LinkPreview> {
    const kind = standIns.listingKind(link)
    return new Promise((resolve, reject) =>
      setTimeout(() => {
        if (link.includes("#P!") && password !== "secret") {
          reject({ kind: "passwordNeeded", wrong: password !== null, tried: 0 })
        } else if (link.includes("fail")) {
          reject({ kind: "unavailable", detail: "[youtube] aaaaaaaaaaa: This video is unavailable" })
        } else if (link.includes("images")) {
          reject({ kind: "imagesNotSetUp" })
        } else if (kind) {
          resolve({ kind: "listing", listing: listingStandIn(kind, isOnX(link), isBig(link)), skipped: [] })
        } else if (link.includes("instagram")) {
          resolve({ kind: "video", ...postStandIn })
        } else if (link.includes("gofile.io")) {
          resolve({ kind: "video", ...(link.toLowerCase().includes("again") ? filePostAgainStandIn : filePostStandIn) })
        } else if (link.includes("mega.nz")) {
          const folder = link.toLowerCase().includes("again")
            ? sharedFolderAgainStandIn
            : folderSize
              ? sharedFolderStandInOf(folderSize)
              : sharedFolderStandIn
          resolve({ kind: "video", ...folder })
        } else {
          resolve({ kind: "video", ...previewStandIn })
        }
      }, kind ? 1600 : 800),
    )
  },
  /**
   * The items after a stand-in listing's first ones, a batch every 600 ms: 50 for Load More (48 on the account), and
   * for Load New and Load All batches of 100 (96 on the account) up to 1,000 videos on the channel or the account's
   * 8,634 posts, Load New until it has as many new ones as asked. The X account has 369 posts, which no batch says: its
   * 372 photos and videos are all the site counts; the big one (a link containing "big") has 2,400, and X makes its
   * read wait 20 seconds after each 400. A link containing "fail" fails after the first batch; `cancelLoadingMore`
   * stops it.
   */
  loadMoreOfListing(
    more: ListingMore,
    read: MoreRead,
    onItems: (batch: MoreItems) => void,
    onPause: (pause: { site: string; until: number } | null) => void = () => {},
  ): Promise<void> {
    const onX = isOnX(more.link)
    const big = onX && isBig(more.link)
    const account = onX || more.link.includes("instagram.com")
    const total = big ? 2400 : onX ? 369 : account ? 8634 : 1000
    const all = read.kind !== "next"
    const step = onX ? (all ? 100 : 50) : account ? (all ? 96 : 48) : all ? 100 : 50
    loadingMore.stopped = false
    return new Promise((resolve, reject) => {
      let index = more.read
      let found = 0
      let sinceWait = 0
      const next = () => {
        if (loadingMore.stopped) {
          reject({ kind: "cancelled" })
          return
        }
        if (big && sinceWait >= 400) {
          sinceWait = 0
          onPause({ site: "X", until: Date.now() + 20_000 })
          loadingMore.timer = setTimeout(() => {
            if (!loadingMore.stopped) onPause(null)
            next()
          }, 20_000)
          return
        }
        if (more.link.includes("fail") && index > more.read) {
          reject({ kind: "tooManyRequests", detail: "429", site: "Instagram", until: Date.now() + 5 * 60_000 })
          return
        }
        const count = Math.min(step, total - index)
        const items = Array.from({ length: count }, (_, at) => listedStandIn(account ? "account" : "channel", index + at))
        index += count
        sinceWait += count
        found += items.filter((item) => item.lastDownload === null && !item.inQueue).length
        const ended = index >= total
        const said = onX ? null : account || ended ? total : null
        onItems({ items, more: ended ? null : { ...more, read: index }, total: said })
        if (ended || read.kind === "next" || (read.kind === "new" && found >= read.count)) resolve()
        else loadingMore.timer = setTimeout(next, 600)
      }
      loadingMore.timer = setTimeout(next, 600)
    })
  },
  cancelLoadingMore(): Promise<void> {
    loadingMore.stopped = true
    return Promise.resolve()
  },
  /** A channel's 1,000 videos, after a moment; `?count=fail` fails it. */
  countListing(more: ListingMore): Promise<number | null> {
    if (!more.channelId) return Promise.resolve(null)
    return new Promise((resolve, reject) =>
      setTimeout(() => {
        if (window.location.search.includes("count=fail")) {
          reject({ kind: "failed", detail: "yt-dlp stopped" })
        } else {
          resolve(1000)
        }
      }, 1500),
    )
  },
  /** As the core tells them: a YouTube channel (`/@name`) or playlist (`/playlist?list=`), an Instagram or X account. */
  listingKind(link: string): ListingKind | null {
    if (/youtube\.com\/(@[^/?#]+|channel\/|c\/|user\/)/.test(link)) return "channel"
    if (/youtube\.com\/playlist\?list=/.test(link)) return "playlist"
    if (/instagram\.com\/(?!p\/|reels?\/|stories\/|explore\/)[\w.]+\/?(posts\/?|reels\/?)?$/.test(link)) return "account"
    if (isOnX(link) && /\.com\/(?!i\/|home\b|explore\b|search\b)\w{1,15}\/?(media\/?)?$/.test(link)) return "account"
    return null
  },
}

/** Whether a link is on X, whose accounts count their photos and videos rather than their posts. */
/** The stand-in X account of thousands of posts, which X lists a stretch at a time. */
function isBig(link: string): boolean {
  return link.includes("big")
}

function isOnX(link: string): boolean {
  return /^(https?:\/\/)?(www\.)?(x|twitter)\.com\//.test(link)
}

/** A listing with a downloaded item and a queued one: 60 for a channel or an account, 300 for a playlist, to check a
 * long list. */
/** Watched sources on three sites: a channel with new and skipped videos, a quiet channel, a playlist being checked,
 * an Instagram account whose last check failed and an X account with new posts, one of them found by the account too. */
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
      picture: null,
      folder: "D:\\Videos\\{uploader}",
      presetId: "best-audio",
      newItemsStart: null,
      unpack: null,
      addedAt: now - 7 * 86_400,
      lastCheckedAt: now - 2 * 3600,
      lastError: null,
      checking: false,
      downloadedCount: 12,
      latestPost: "2026-11-12",
      newItems: channel.items.slice(0, 3).map(toItem),
      skippedItems: channel.items.slice(5, 7).map(toItem),
    },
    {
      id: 3,
      url: "https://www.youtube.com/@quiet/videos",
      kind: "channel",
      name: "A Quiet Channel",
      siteName: "youtube.com",
      thumbnailUrl: channel.thumbnailUrl,
      picture: null,
      folder: null,
      presetId: null,
      newItemsStart: null,
      unpack: null,
      addedAt: now - 30 * 86_400,
      lastCheckedAt: now - 3 * 86_400,
      lastError: null,
      checking: false,
      downloadedCount: 4,
      latestPost: "2025-06-02",
      newItems: [],
      skippedItems: [],
    },
    {
      id: 4,
      url: "https://www.youtube.com/playlist?list=PLstandin",
      kind: "playlist",
      name: "A Long Playlist",
      siteName: "youtube.com",
      thumbnailUrl: channel.thumbnailUrl,
      picture: null,
      folder: null,
      presetId: null,
      newItemsStart: null,
      unpack: null,
      addedAt: now - 10 * 86_400,
      lastCheckedAt: null,
      lastError: null,
      checking: true,
      downloadedCount: 0,
      latestPost: null,
      newItems: [],
      skippedItems: [],
    },
    {
      id: 5,
      url: "https://x.com/someone/media",
      kind: "account",
      name: "Someone on X",
      siteName: "x.com",
      thumbnailUrl: account.thumbnailUrl,
      picture: null,
      folder: "X\\{account}",
      presetId: null,
      newItemsStart: "ticked",
      unpack: "inPlace",
      addedAt: now - 5 * 86_400,
      lastCheckedAt: now - 20 * 60,
      lastError: null,
      checking: false,
      downloadedCount: 0,
      latestPost: "2026-11-13",
      newItems: [...account.items.slice(0, 4), channel.items[1]].map(toItem),
      skippedItems: [],
    },
    {
      id: 2,
      url: "https://www.instagram.com/someaccount/posts/",
      kind: "account",
      name: account.title,
      siteName: account.siteName,
      thumbnailUrl: account.thumbnailUrl,
      picture: null,
      folder: null,
      presetId: null,
      newItemsStart: null,
      unpack: null,
      addedAt: now - 2 * 86_400,
      lastCheckedAt: now - 600,
      lastError: { kind: "signInRequired", detail: "login required", site: "Instagram", cookies: "notSignedIn" },
      checking: false,
      downloadedCount: 3,
      latestPost: "2026-10-30",
      newItems: [],
      skippedItems: [],
    },
  ]
}

/** The stand-in's Load More or Load All in progress, which `cancelLoadingMore` stops. */
const loadingMore: { stopped: boolean; timer: ReturnType<typeof setTimeout> | null } = { stopped: false, timer: null }

/** A channel, a playlist or an account: Instagram's, which says how many posts it has, or with `onX` X's, which says
 * how many photos and videos instead: 372, or 28,260 on the `big` one. */
function listingStandIn(kind: ListingKind, onX = false, big = false): Listing {
  const posts = kind === "account"
  const count = kind === "playlist" ? 300 : 50
  if (posts && onX) {
    const link = big ? "https://x.com/bigaccount/media" : "https://x.com/someone/media"
    const title = big ? "A Big Account on X" : "Someone on X"
    const place = (skip: number) => ({ cursor: null, skip })
    return {
      kind,
      title,
      uploader: null,
      siteName: "x.com",
      url: big ? "https://x.com/bigaccount" : "https://x.com/someone",
      thumbnailUrl: previewStandIn.video.thumbnailUrl,
      total: null,
      mediaTotal: big ? 28_260 : 372,
      pace: { items: 1000, pauseSeconds: 900 },
      newestOnly: true,
      items: Array.from({ length: count }, (_, index) => listedStandIn(kind, index)),
      more: {
        link,
        title,
        read: count,
        cursor: null,
        channelId: null,
        media: { photos: place(40), videos: place(10), handedDownTo: null },
      },
    }
  }
  const title = { channel: "jawed", playlist: "A Long Playlist", account: "Some Account" }[kind]
  const link = posts ? "https://www.instagram.com/someaccount/posts/" : "https://www.youtube.com/@jawed/videos"
  return {
    kind,
    title,
    uploader: kind === "playlist" ? "jawed" : null,
    siteName: posts ? "instagram.com" : "youtube.com",
    url: posts ? "https://www.instagram.com/someaccount/" : link,
    thumbnailUrl: previewStandIn.video.thumbnailUrl,
    total: posts ? 8634 : null,
    mediaTotal: null,
    pace: null,
    newestOnly: kind !== "playlist",
    items: Array.from({ length: count }, (_, index) => listedStandIn(kind, index)),
    more:
      kind === "playlist"
        ? null
        : { link, title, read: count, cursor: null, channelId: posts ? null : "UC4QobU6STFB0P71PMvOGN5A", media: null },
  }
}

/** A stand-in listing's item at `index` in its order, newest first: the fifth queued; on the account, all but the 3
 * newest of its first 150 posts downloaded, as if the newest were downloaded once before, else every 7th from the
 * third. */
function listedStandIn(kind: ListingKind, index: number): ListedItem {
  const posts = kind === "account"
  const daysAgo = (days: number) => new Date(Date.now() - days * 86_400_000).toISOString().slice(0, 10)
  const base = posts ? postStandIn.video : previewStandIn.video
  return {
    video: {
      ...base,
      videoId: `listed${index}`,
      url: `${base.url}#${index}`,
      title: posts ? `Post number ${index + 1}, with a caption long enough to need cutting short` : `Video number ${index + 1}`,
      thumbnailUrl: posts ? standInPicture(index) : base.thumbnailUrl,
      post: posts ? STAND_IN_POSTS[index % STAND_IN_POSTS.length] : null,
      durationSeconds: posts ? null : 45 + index * 97,
      uploadDate: posts ? daysAgo(index * 3) : null,
    },
    approximateDate: posts ? null : daysAgo(index * 5),
    lastDownload: (posts ? index >= 3 && index < 150 : index % 7 === 2) ? previewStandIn.lastDownload : null,
    inQueue: index === 4,
  }
}

/** What an account's posts hold, in turn: images, a video, an image, both, so the type filter has types to offer. */
const STAND_IN_POSTS = [
  { images: 3, videos: 0, others: 0 },
  { images: 0, videos: 1, others: 0 },
  { images: 1, videos: 0, others: 0 },
  { images: 2, videos: 1, others: 0 },
]

/** The shapes an account's posts come in (square, portrait, a reel's, landscape), as width over height. */
const STAND_IN_SHAPES = [1, 4 / 5, 9 / 16, 1.91, 3 / 4, 4 / 5, 1, 16 / 9]

/** A picture made here, numbered and in one of the shapes posts come in, so Masonry and Gallery can be checked in a
 * browser without the network. */
function standInPicture(index: number): string {
  const ratio = STAND_IN_SHAPES[index % STAND_IN_SHAPES.length]
  const width = 400
  const height = Math.round(width / ratio)
  const hue = (index * 47) % 360
  const svg =
    `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}">` +
    `<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1">` +
    `<stop offset="0" stop-color="hsl(${hue} 55% 62%)"/><stop offset="1" stop-color="hsl(${(hue + 60) % 360} 50% 38%)"/>` +
    `</linearGradient></defs><rect width="100%" height="100%" fill="url(#g)"/>` +
    `<text x="50%" y="50%" font-family="sans-serif" font-size="96" fill="white" text-anchor="middle" dominant-baseline="central">${index + 1}</text></svg>`
  return `data:image/svg+xml,${encodeURIComponent(svg)}`
}

/**
 * Every piece of UI text, in English, kept here so translations can come later. Never write UI text in a component.
 * Labels are Title Case; sentences (descriptions, messages) are sentence case. See DESIGN.md → Labels and Casing.
 */
export const strings = {
  appName: "Tsundoku",
  appNameJapanese: "積ん読",
  version: (version: string) => `Version ${version}`,
  units: {
    bytes: ["B", "KB", "MB", "GB", "TB"],
  },
  nav: {
    queue: "Queue",
    history: "History",
    presets: "Presets",
    settings: "Settings",
  },
  addBar: {
    placeholder: "Paste a Video Link",
    add: "Add",
  },
  queue: {
    emptyTitle: "Nothing in the Queue",
    emptyBody: "Paste a video link above to download it.",
  },
  history: {
    emptyTitle: "No Downloads Yet",
    emptyBody: "Everything you download is listed here, so you never get the same video twice.",
  },
  tools: {
    names: {
      ytDlp: "yt-dlp",
      ffmpeg: "ffmpeg",
      deno: "Deno",
    },
    descriptions: {
      ytDlp: "Downloads the videos.",
      ffmpeg: "Merges video and audio, and converts formats. Comes with ffprobe.",
      deno: "Runs YouTube's player code for yt-dlp. Without it, some YouTube formats may be missing.",
    },
    managed: "Tsundoku's Copy",
    external: "Your Copy",
    notSetUp: "Not set up.",
    stages: {
      connecting: "Connecting to GitHub…",
      downloading: (received: string, total: string | null) =>
        total === null ? `Downloading… ${received}` : `Downloading… ${received} of ${total}`,
      verifying: "Checking the download…",
      installing: "Installing…",
      checking: "Making sure it runs…",
      updating: "Updating…",
    },
    errors: {
      network: "Couldn't reach GitHub. Check your connection, then try again.",
      rateLimited: "GitHub is limiting requests from this network. Try again in an hour.",
      missingFromRelease: (file: string) => `The latest release is missing ${file}. Try again later.`,
      checksumMissing: (file: string) =>
        `The latest release lists no checksum for ${file}, so it can't be checked. Try again later.`,
      checksumMismatch: "The download doesn't match its checksum, so it wasn't installed. Try again.",
      blocked: (name: string) =>
        `Windows Security or your antivirus blocked or removed ${name}. Allow it there, then try again.`,
      fileMissing: "The file isn't there anymore.",
      ffprobeMissing: "ffprobe.exe must be in the same folder as ffmpeg.exe.",
      notThisTool: (name: string) => `This file isn't ${name}.`,
      wontRun: (name: string) => `${name} won't run.`,
      updateFailed: (name: string) => `${name} couldn't update.`,
      busy: (name: string) => `${name} is already being set up or updated.`,
      cancelled: "Cancelled.",
      notManaged: (name: string) => `Only Tsundoku's own copy of ${name} can be updated.`,
      storage: "Couldn't write the tool's files.",
      unexpected: "Something went wrong.",
    },
  },
  setup: {
    title: "Set Up Your Tools",
    body: "Tsundoku downloads videos with three free tools. Use copies you already have, or let Tsundoku download its own and keep them up to date.",
    lookingForTools: "Looking for tools on this PC…",
    useThisCopy: "Use This Copy",
    foundDetail: (version: string, path: string) => `Version ${version} · ${path}`,
    downloadFresh: "Download Fresh",
    downloadDetail: (size: string | null) =>
      size === null ? "From GitHub. Tsundoku keeps it up to date." : `${size} from GitHub. Tsundoku keeps it up to date.`,
    notFound: (size: string | null) =>
      size === null
        ? "Not found on this PC, so Tsundoku will download it."
        : `Not found on this PC, so Tsundoku will download it (${size}).`,
    ready: (version: string | null) => (version === null ? "Ready." : `Ready: version ${version}.`),
    continue: "Continue",
    downloadAndContinue: "Download and Continue",
    tryAgain: "Try Again",
    cancel: "Cancel",
    continueWithoutDeno: "Continue Without Deno",
  },
  settings: {
    tools: "Tools",
    loadingTools: "Checking the tools…",
    updateNow: "Update Now",
    checkForUpdates: "Check for Updates",
    update: "Update",
    download: "Download",
    useOwnCopy: "Use Tsundoku's Copy",
    chooseFile: "Choose File…",
    chooseFileTitle: (name: string) => `Choose ${name}`,
    programsFilter: "Programs",
    cancel: "Cancel",
    upToDate: "Up to date.",
    updatedTo: (version: string | null) => (version === null ? "Updated." : `Updated to version ${version}.`),
    installed: (version: string | null) => (version === null ? "Installed." : `Installed version ${version}.`),
    updateAvailable: (latest: string) => `Update available: ${latest}.`,
    channel: "Update Channel",
    channels: {
      stable: "Stable",
      nightly: "Nightly",
    },
    channelHint: "Nightly gets fixes for sites sooner, but is tested less.",
    about: "About",
  },
} as const

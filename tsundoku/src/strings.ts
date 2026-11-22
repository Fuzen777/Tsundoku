/**
 * Every piece of UI text, in English, kept here so translations can come later. Never write UI text in a component.
 * Labels are Title Case; sentences (descriptions, messages) are sentence case. See DESIGN.md → Labels and Casing.
 */
/** "a", "a and b", "a, b and c". */
function listOf(items: string[]): string {
  return items.length < 2 ? items.join("") : `${items.slice(0, -1).join(", ")} and ${items[items.length - 1]}`
}

/** An X account's photos and videos, as the site counts them: "372 Photos & Videos" on a button (`label`), "372 photos
 * & videos" in a line of details, or "372 photos and videos" in a sentence. */
function photosAndVideos(count: number, style: "label" | "details" | "sentence"): string {
  if (count === 1) return style === "label" ? "1 Photo or Video" : "1 photo or video"
  const words = { label: "Photos & Videos", details: "photos & videos", sentence: "photos and videos" }[style]
  return `${count.toLocaleString()} ${words}`
}

export const strings = {
  appName: "Tsundoku",
  appNameJapanese: "積ん読",
  version: (version: string) => `Version ${version}`,
  versionPortable: (version: string) => `Version ${version} · Portable`,
  units: {
    bytes: ["B", "KB", "MB", "GB", "TB"],
    perSecond: (amount: string) => `${amount}/s`,
  },
  /** Between the parts of a line of details: "jawed · youtube.com". */
  separator: " · ",
  errors: {
    cancelled: "Cancelled.",
    unexpected: "Something went wrong.",
  },
  dialogs: {
    cancel: "Cancel",
  },
  /** The buttons that put a placeholder in a field (PlaceholderButtons.tsx). */
  placeholders: {
    insert: "Insert",
  },
  /** A post's files, picked in a strip (PostFilesStrip.tsx) on the preview card and in Pending, or in a list
   * (PostFilesPicker.tsx) for a post of files that aren't all pictures. */
  postFiles: {
    count: (picked: number, total: number) => `${picked} of ${total} Files`,
    summary: (picked: number, total: number) => `${picked} of ${total} files`,
    selectAll: "Select All Files",
    file: (number: number, video: boolean) => `${video ? "Video" : "Image"} ${number}`,
    /** A file of a post of files (a folder on Gofile or Pixeldrain) when the site gives no name. */
    namelessFile: (number: number) => `File ${number}`,
    downloaded: "Downloaded",
    downloadedHint: "Downloaded before",
    notPicked: "Not Picked",
    notPickedHint: "Not picked last time",
    leftOut: (count: number) =>
      count === 1
        ? "1 file wasn't picked last time. It's the one ticked, so downloading again gets just that one."
        : `${count} files weren't picked last time. They're the ones ticked, so downloading again gets just those.`,
    leftOutCount: (count: number) =>
      count === 1 ? "1 file wasn't picked last time." : `${count} files weren't picked last time.`,
    pickFiles: "Pick Files",
    hideFiles: "Hide Files",
  },
  /** A shared folder's files, picked in a list (SharedFilesList.tsx) on the preview card and in Pending. */
  sharedFiles: {
    count: (picked: number, total: number, size: string) => `${picked} of ${total} Files · ${size}`,
    summary: (picked: number, total: number, size: string) => `${picked} of ${total} files, ${size}`,
    selectAll: "Select All Files",
    selectShown: "Select the Files Shown",
    filter: "Filter Files",
    noMatches: "No files match.",
    folderContents: (count: number, size: string) => `${count} ${count === 1 ? "file" : "files"} · ${size}`,
    expand: (folder: string) => `Expand ${folder}`,
    collapse: (folder: string) => `Collapse ${folder}`,
    downloaded: "Downloaded",
    downloadedHint: "Downloaded before",
    notDownloaded: "Not Downloaded",
    notDownloadedHint: "Not downloaded before: left out, or added to the folder since",
    leftOut: (count: number) =>
      count === 1
        ? "1 file of this folder wasn't downloaded before. It's the one ticked, so downloading again gets just that one."
        : `${count} files of this folder weren't downloaded before. They're the ones ticked, so downloading again gets just those.`,
    leftOutCount: (count: number) =>
      count === 1
        ? "1 file of this folder wasn't downloaded before."
        : `${count} files of this folder weren't downloaded before.`,
  },
  /** The toast that offers to undo a removal (UndoToast.tsx). */
  undoToast: {
    undo: "Undo",
    dismiss: "Dismiss",
  },
  nav: {
    /** The downloads added and not cleared yet, running or done (`QueueView`), apart from History's, which keeps every
     * download ever made. */
    queue: "Downloads",
    pending: "Pending",
    watched: "Watched",
    history: "History",
    presets: "Presets",
    rules: "Rules",
    settings: "Settings",
    /** The list of a view's chapters under it in the sidebar. */
    chapters: (view: string) => `Jump Within ${view}`,
  },
  /** The window's keyboard shortcuts (shortcuts.ts), as hover texts and Settings → Keyboard Shortcuts show them. */
  shortcuts: {
    /** A control's hover text with its shortcut: "Downloads (Ctrl+1)". */
    withKeys: (label: string, keys: string) => `${label} (${keys})`,
    paste: "Ctrl+V",
    pasteAnywhere: "Ctrl+V Anywhere",
    typeLink: "Ctrl+L",
    closeField: "Esc",
    view: (number: number) => `Ctrl+${number}`,
    views: (last: number) => `Ctrl+1 to Ctrl+${last}`,
    delete: "Delete",
  },
  /** The ways to add links, as the empty Queue and Pending list them (WaysToAdd). */
  waysToAdd: {
    paste: "Paste a Link",
    pasteState: "Ctrl+V anywhere, or Paste Link in the sidebar.",
    pasteAction: "Paste",
    drop: "Drop Links or Files",
    dropState: "From a browser, or a text file, bookmarks export or shortcut.",
    dropAction: "Choose File…",
    watch: "Watch Clipboard",
    watchOn: (rightAway: boolean) =>
      rightAway ? "Links you copy in any app start downloading once they're read." : "Links you copy in any app come here.",
    watchOff: "Catches links you copy in any app.",
    watchAction: "Turn On",
    on: "On",
    shortcut: "Clipboard Shortcut",
    shortcutState: (keys: string) => `${keys} adds the links you copied, from any app.`,
    shortcutNone: "Keys that add the links you copied, from any app.",
    bookmark: "Send from Your Browser",
    bookmarkState: "A bookmark that sends the page you're on.",
    sendButtons: "Send Buttons on Instagram, X and YouTube",
    sendButtonsState: "A button on each post and video that sends it here.",
    setUp: "Set Up",
    change: "Change",
  },
  addBar: {
    /** The button at the top of the sidebar, which adds the links the clipboard holds. */
    paste: "Paste Link",
    pasteHint: "Paste One Link or Several",
    /** Under Paste Link when the clipboard holds no link, or nothing that's text. */
    noLinkCopied: "There's no link on the clipboard.",
    /** Beside Paste Link: opens a field under it to type or edit a link. */
    typeLink: "Type a Link",
    placeholder: "Type or Paste a Link",
    add: "Add",
    close: "Close",
    errors: {
      dismiss: "Dismiss",
    },
    /** A file of links dropped on the window. */
    files: {
      unsupportedType: "Tsundoku reads links from .txt, .html and .url files.",
      tooLarge: (name: string, size: string) => `${name} is too large to read links from: the most is ${size}.`,
      noLinks: (name: string) => `No links found in ${name}.`,
      unreadable: (name: string) => `Couldn't read ${name}.`,
      chooseTitle: "Choose Files of Links",
      chooseFilter: "Text Files, Bookmarks and Shortcuts",
      manyTitle: (count: number) => `Add ${count.toLocaleString()} Links?`,
      /** `files`: the names of the files the links came from. */
      manyMessage: (files: string[], count: number, duration: string) =>
        `${files.length === 1 ? `${files[0]} holds` : `These ${files.length} files hold`} ${count.toLocaleString()} links. Tsundoku checks each one, which takes ${duration} for this many, and the ones with nothing to download leave on their own.`,
      addThem: "Add Them",
      aboutMinutes: (minutes: number) => (minutes === 1 ? "about a minute" : `about ${minutes} minutes`),
      aboutHours: (hours: number) => (hours === 1 ? "about an hour" : `about ${hours.toLocaleString()} hours`),
    },
  },
  queue: {
    emptyTitle: "Nothing in Downloads",
    emptyBody: "Add a link to a video, a post of images or a shared file, and it downloads here.",
    clearFinished: "Clear Finished",
    pauseAll: "Pause All",
    resumeAll: "Resume All",
    retryFailed: "Retry Failed",
    moreActions: "More Actions",
    downloadNext: "Download Next",
    changeFolder: "Change Folder…",
    usualFolder: "Use the Usual Folder",
    changeFolderTitle: "Choose Where This Download Goes",
    changeFolderStoppedTitle: "Start Over in Another Folder?",
    changeFolderStoppedMessage: "What's downloaded so far is in the current folder, so the download starts over.",
    changeFolderConfirm: "Start Over",
    stages: {
      waiting: "Waiting",
      starting: "Starting",
      downloadingVideo: "Downloading Video",
      downloadingAudio: "Downloading Audio",
      downloading: "Downloading",
      merging: "Merging",
      converting: "Converting",
      embedding: "Embedding",
      processing: "Processing",
      finishing: "Finishing",
      countdown: "Counting Down",
      unpacking: "Unpacking",
      done: "Done",
      stopped: "Stopped",
      failed: "Failed",
    },
    stop: "Stop",
    resume: "Resume",
    cancel: "Cancel",
    retry: "Retry",
    tryNow: "Try Now",
    openCheck: "Open Check",
    enterPassword: "Enter Password",
    openLog: "Open Log",
    showLog: "Show Log",
    hideLog: "Hide Log",
    log: {
      loading: "Reading the log…",
      empty: "Nothing written yet.",
      cut: "The log is long: this is its end.",
      copy: "Copy Log",
      openInEditor: "Open in Editor",
    },
    copyDetails: "Copy Details",
    detailsCopied: "Copied",
    openFile: "Open File",
    showInFolder: "Show in Folder",
    progress: (downloaded: string, total: string) => `${downloaded} of ${total}`,
    timeLeft: (time: string) => `${time} left`,
    count: (count: number) => `${count} ${count === 1 ? "download" : "downloads"}`,
    cleared: (count: number) => `Cleared ${count} finished ${count === 1 ? "download" : "downloads"}`,
    removeFinished: "Remove from Downloads",
    removed: (title: string) => `Removed “${title}” from Downloads`,
    groups: {
      running: "Downloading",
      waiting: "Waiting",
      stopped: "Stopped",
      failed: "Failed",
      done: "Done",
    },
    /** Downloads as pictures, in Masonry or Gallery (QueueTiles). */
    /** The heading over the downloads picked together from one channel, playlist or account (QueueSourceGroup). */
    source: {
      done: (done: number, total: number) => `${done} of ${total} done`,
      allDone: (count: number) => `${count} done`,
      states: {
        running: (count: number) => `${count} downloading`,
        waiting: (count: number) => `${count} waiting`,
        stopped: (count: number) => `${count} stopped`,
        failed: (count: number) => `${count} failed`,
      },
      progress: (name: string) => `Downloading from ${name}`,
      pauseAll: (name: string) => `Pause All from ${name}`,
      resumeAll: (name: string) => `Resume All from ${name}`,
      retryFailed: (name: string) => `Retry Failed from ${name}`,
      cancelAll: (name: string) => `Cancel All from ${name}`,
      clearFinished: (name: string) => `Clear Finished from ${name}`,
      cancelTitle: (count: number, name: string) =>
        `Cancel ${count} ${count === 1 ? "Download" : "Downloads"} from ${name}?`,
      cancelMessage: (count: number, finished: number) =>
        `${count === 1 ? "What it has" : "What they have"} downloaded so far is deleted.${
          finished === 0 ? "" : ` The ${finished} finished ${finished === 1 ? "one stays" : "ones stay"} in Downloads.`
        }`,
      cancelConfirm: (count: number) => (count === 1 ? "Cancel Download" : "Cancel Downloads"),
      keep: (count: number) => (count === 1 ? "Keep It" : "Keep Them"),
    },
    /** Why a waiting download, or a link waiting in Pending, doesn't start yet. */
    holds: {
      slowDown: (site: string, time: string) => `${site} asked to slow down. Continuing at ${time}.`,
      hourlyLimit: (site: string, limit: number, time: string) =>
        `${limit} ${site} ${limit === 1 ? "file" : "files"} in the past hour, the limit set in Settings. Continuing at ${time}.`,
      retry: (attempt: number, attempts: number, time: string) =>
        `The connection dropped. Trying again at ${time} (${attempt} of ${attempts}).`,
      limitReached: (site: string, time: string) => `${site}'s download limit is used up. Continuing at ${time}.`,
      notEnoughRoom: (drive: string, free: string, needed: string) =>
        `${drive} has ${free} free, and this needs ${needed}. It starts by itself once there's room.`,
      notEnoughRoomBeside: (drive: string, free: string, needed: string, others: string) =>
        `${drive} has ${free} free, and this needs ${needed} besides the ${others} the downloads running there still need. It starts by itself once there's room.`,
      browserCheck: (site: string, files: number) =>
        files > 1
          ? `${site} wants to check you're a person before ${files} of these files. They wait here until you open the check and start each one's download on ${site}'s page.`
          : `${site} wants to check you're a person before this download. It waits here until you open the check and start the download on ${site}'s page.`,
      windowDownload: (site: string) =>
        `Downloading in ${site}'s check window, which saves the file itself. Stopping this download stops it.`,
      needsPassword: (wrong: boolean, tried: number) =>
        wrong
          ? "The password didn't open this download. It waits here until you enter the right one."
          : tried === 0
            ? "This download needs a password. It waits here until you enter it."
            : `This download needs a password, and ${
                tried === 1 ? "the one Tsundoku tried didn't open it" : `none of the ${tried} Tsundoku tried opened it`
              }. It waits here until you enter it.`,
    },
    closeTitle: "Downloads Are Running",
    closeMessage: (count: number) =>
      `${count === 1 ? "A download is" : `${count} downloads are`} still running. Close Tsundoku anyway? Next time, ${
        count === 1 ? "it" : "they"
      } can continue where ${count === 1 ? "it" : "they"} stopped.`,
    closeConfirm: "Close",
    errors: {
      folder: "Couldn't create the download folder.",
      noFile: "The download finished without saying which file it made.",
      alreadyQueued: "This is already in Downloads.",
      notFound: "This download isn't in Downloads anymore.",
      wrongState: "That can't be done to this download right now.",
      damaged: (file: string) => `${file} arrived damaged and was deleted. Retry to download it again.`,
      windowDownloadStopped: (site: string) =>
        `The download in ${site}'s check window stopped before the file arrived. Retry to open the check again.`,
      storage: "Couldn't save Downloads or the history.",
      logMissing: "This download has no log yet.",
      invalidRateLimit: "Use a number of bytes per second, with K, M or G after it if you like, such as 500K or 2M.",
      nothingPicked: "Tick at least one file to download it.",
      pickedFilesGone:
        "The files picked are no longer in the shared folder: they were moved, renamed or deleted. Paste the link again to pick from what it holds now.",
      shortcutTaken: (keys: string) => `Another app already uses ${keys}. Pick another shortcut.`,
      unpack: {
        needsPassword: (archive: string) =>
          `${archive} has a password. Enter it here: it's unpacked without downloading again.`,
        wrongPassword: (archive: string, tried: number) =>
          `${
            tried === 1 ? "The password Tsundoku tried doesn't open" : `None of the ${tried} passwords Tsundoku tried opens`
          } ${archive}. Enter the right one here: it's unpacked without downloading again.`,
        missingPart: (archive: string, part: string) =>
          `${archive} goes on in ${part}, which isn't in its folder. Once it is, Retry unpacks it without downloading again.`,
        damaged: (archive: string) => `${archive} is damaged, so it can't be unpacked. It's kept as it is.`,
        unsupported: (archive: string) => `Tsundoku can't unpack ${archive}. It's kept as it is.`,
        noRoom: (archive: string, drive: string, needed: string, free: string) =>
          `${archive} holds ${needed}, and ${drive} has ${free} free. Make room, then Retry: it's unpacked without downloading again.`,
        write: (archive: string) => `Couldn't write what ${archive} holds. Retry unpacks it without downloading again.`,
      },
    },
    fixes: {
      updateYtDlp: "Update yt-dlp",
      updateGalleryDl: "Update gallery-dl",
      setUpImages: "Set Up Image Downloads",
      installed: "Done. Try again.",
      tryNightly: "Try Nightly",
      setUpCookies: "Set Up Cookies",
      tryAgain: "Try Again",
      cookieSettings: "Cookie Settings",
      updated: (version: string | null) =>
        version === null ? "yt-dlp is updated. Try again." : `yt-dlp is now version ${version}. Try again.`,
      passwordSettings: "Password Settings",
      /** Under an archive's password field: what becomes of the password typed, by Save Past Passwords. */
      archivePasswordKept: "Once it opens the archive, it's kept for the next ones: Save Past Passwords is on.",
      archivePasswordNotKept:
        "It's used for this download only. To have it tried on the next archives too, add it to your passwords in Settings, or turn on Save Past Passwords there.",
    },
  },
  history: {
    emptyTitle: "No Downloads Yet",
    emptyBody: "Everything you download is listed here, so you never download the same thing twice.",
    noMatchesTitle: "No Matches",
    noMatchesBody: "No download matches this search and these filters.",
    searchPlaceholder: "Search Titles and Uploaders",
    allSites: "All Sites",
    allPresets: "All Presets",
    /** The filter of a watched source's items, which Show Downloads in History on its card picks. */
    allSources: "All Watched Sources",
    sortBy: "Sort By",
    sorts: {
      downloaded: "Date Downloaded",
      title: "Title",
      uploader: "Uploader",
      site: "Site",
      duration: "Length",
      size: "Size",
      preset: "Preset",
    },
    ascending: "Ascending",
    descending: "Descending",
    itemCount: (count: number) => (count === 1 ? "1 item" : `${count} items`),
    /** What the items listed take on the disk, after their count. */
    sizeOnDisk: (size: string) => `${size} on disk`,
    noFilesOnDisk: "no files left on disk",
    /** What Tsundoku keeps for the whole history in its data folder. */
    ownData: (size: string) => `Tsundoku's own data: ${size}`,
    ownDataHint: (parts: {
      database: string
      thumbnails: string
      thumbnailCount: number
      logs: string
      logCount: number
      archive: string
    }) =>
      `For the whole history: its database (${parts.database}), ` +
      `${parts.thumbnailCount === 1 ? "1 kept thumbnail" : `${parts.thumbnailCount} kept thumbnails`} (${parts.thumbnails}), ` +
      `${parts.logCount === 1 ? "1 download log" : `${parts.logCount} download logs`}, kept 30 days (${parts.logs}), ` +
      `and yt-dlp's archive (${parts.archive}). The tools and settings aren't counted.`,
    sizeOnDiskHint: (files: number, missing: number) =>
      files === 0
        ? "None of the files their downloads made is where it was saved anymore."
        : `${files === 1 ? "1 file" : `${files} files`} from every download of these items, still where they were saved.` +
          (missing === 0 ? "" : ` ${missing} more ${missing === 1 ? "isn't" : "aren't"} there anymore.`),
    downloadCount: (count: number) => `${count} Downloads`,
    fileCount: (count: number) => `${count} Files`,
    fileMissing: "File Missing",
    openFile: "Open File",
    showInFolder: "Show in Folder",
    moreActions: "More Actions",
    openSourcePage: "Open Source Page",
    downloadAgain: "Download Again",
    forget: "Forget",
    forgetTitle: "Forget This Item?",
    forgetMessage: (title: string) =>
      `${title} will leave the history, so Tsundoku won't warn before it's downloaded again. Its files stay where they are.`,
    allKinds: "All Kinds",
    kinds: {
      video: "Videos",
      audio: "Audio",
      post: "Posts",
      file: "Shared Files",
    },
    dateRanges: {
      any: "Any Time",
      today: "Today",
      week: "Past 7 Days",
      month: "Past 30 Days",
      year: "Past Year",
      custom: "Custom…",
    },
    dateRange: "Downloaded",
    dateFrom: "From",
    dateTo: "To",
    fileStates: {
      label: "Files",
      any: "All Files",
      present: "Only Existing Files",
      missing: "Only Missing Files",
    },
    selectAll: "Select All",
    select: "Select",
    selectedCount: (count: number) => `${count} selected`,
    forgetSelected: "Forget Selected",
    moveFiles: "Move Files…",
    moveSelected: "Move Selected…",
    moveFilesTitle: (count: number) => (count === 1 ? "Move the Files To" : `Move the Files of ${count} Items To`),
    movedFiles: (moved: number, missing: number) =>
      (moved === 0 ? "No file was left to move." : `Moved ${moved} ${moved === 1 ? "file" : "files"}.`) +
      (missing === 0 ? "" : ` ${missing} ${missing === 1 ? "was" : "were"} missing already.`),
    moveFailed: (count: number) => `${count} ${count === 1 ? "file" : "files"} couldn't be moved.`,
    moveFailure: (path: string, reason: string) => `${path}: ${reason}`,
    showSelectedInFolder: "Show Selected in Folder",
    forgetManyTitle: (count: number) => (count === 1 ? "Forget 1 Item?" : `Forget ${count} Items?`),
    forgetManyMessage:
      "They'll leave the history, so Tsundoku won't warn before they're downloaded again. Their files stay where they are.",
    openFoldersTitle: (count: number) => `Open ${count} Folders?`,
    openFoldersMessage: (count: number) =>
      `The selected downloads are in ${count} folders, and each opens in a File Explorer window of its own.`,
    openFolders: "Open Folders",
    noFilesLeft: "None of the selected downloads' files is where it was saved anymore.",
    errors: {
      database: "Couldn't read the history.",
      newerVersion: "The history was saved by a newer version of Tsundoku. Update Tsundoku to open it.",
      notFound: "This download isn't in the history anymore.",
      fileMissing: "The file isn't where it was saved anymore.",
      archive: "Couldn't update yt-dlp's download archive.",
      openFailed: "Windows couldn't open it.",
    },
  },
  /** The Pending, Downloads and History views' items as pictures (Masonry and Gallery: `ItemTiles`). */
  tiles: {
    hint: "Right-click for its actions, or click for its details.",
    openHint: "Double-click to open it.",
    menu: (title: string) => `Actions for ${title}`,
    showDetails: "Show Details",
    hideDetails: "Hide Details",
  },
  /** The Pending, Downloads and History views' grouping and sorting. */
  lists: {
    groupBy: "Group By",
    groups: {
      none: "None",
      site: "Site",
      destination: "Folder",
      rule: "Rule",
      uploader: "Uploader",
      month: "Month",
    },
    /** The history's videos without an uploader, grouped by uploader. */
    noUploader: "No Uploader",
    byStatus: "Status",
    byStatusHint: "Group by where each one stands first",
    ruleGroup: (rule: string, folder: string) => `${rule} · ${folder}`,
    presetFolderGroup: (folder: string) => `Preset's Folder · ${folder}`,
    downloadFolderGroup: (folder: string) => `Download Folder · ${folder}`,
    /** The download folder's group in the sidebar, without its path. */
    downloadFolderChapter: "Download Folder",
    chosenFolderGroup: (folder: string) => `Picked Folder · ${folder}`,
    /** Downloads added by a rule deleted since. */
    deletedRuleGroup: "Deleted Rule",
    /** Downloads added before Tsundoku kept which rule gave their folder, when the rules give them another now. */
    unknownRuleGroup: (folder: string) => `Unknown Rule · ${folder}`,
    collapse: (group: string) => `Collapse ${group}`,
    expand: (group: string) => `Expand ${group}`,
    selectGroup: (group: string) => `Select Everything in ${group}`,
    sortBy: "Sort By",
    sorts: {
      added: "Date Added",
      title: "Title",
      uploader: "Uploader",
      site: "Site",
    },
    ascending: "Ascending",
    descending: "Descending",
  },
  appUpdate: {
    available: (version: string) => `Tsundoku ${version} is available.`,
    install: "Install and Restart",
    later: "Later",
    downloading: (received: string, total: string | null) =>
      total === null ? `Downloading the update… ${received}` : `Downloading the update… ${received} of ${total}`,
    failed: "Couldn't install the update.",
    check: "Check for Updates",
    upToDate: "Tsundoku is up to date.",
    checkFailed: "Couldn't check for updates.",
    portable: "This portable copy doesn't update itself yet: replace it with a newer one to update.",
    whatsNew: "What's New",
    releaseNotes: "Release Notes",
  },
  /** The card that says what changed after an update, or any version's changes when it's opened from Settings. */
  whatsNew: {
    title: (version: string) => `What's New in Tsundoku ${version}`,
    close: "Close",
    open: "What's New",
    browseTitle: "What's New in Tsundoku",
    version: "Version",
    versionOption: (version: string, date: string | null) => (date === null ? version : `${version} · ${date}`),
    none: "This copy of Tsundoku came without its release notes.",
  },
  pending: {
    emptyTitle: "Nothing Pending",
    emptyBody: "Links you send from your browser, copy, or paste or drop several at once wait here until you start them.",
    watchClipboard: "Watch Clipboard",
    watchClipboardHint: "Catch links you copy in any app",
    checkingCopied: (count: number) => (count === 1 ? "Checking a copied link…" : `Checking ${count} copied links…`),
    checkingFiles: (count: number) =>
      count === 1 ? "Checking a link from a file…" : `Checking ${count.toLocaleString()} links from files…`,
    /** Links a page's Click'n'Load button sent, while they're read. */
    checkingPages: (count: number) =>
      count === 1 ? "Checking a link a page sent…" : `Checking ${count.toLocaleString()} links pages sent…`,
    showMore: (count: number) => `Show ${count.toLocaleString()} More`,
    count: (count: number) => `${count} ${count === 1 ? "link" : "links"}`,
    selectAll: "Select All",
    select: "Select",
    videosStartAs: "Videos Start As",
    videosStartAsHint: "For videos no rule picks a preset for, and that have none of their own.",
    defaultPreset: "Default",
    defaultPresetHint: (preset: string) => `Follows Videos Start As: ${preset}.`,
    startAll: "Start All",
    startSelected: "Start Selected",
    removeSelected: "Remove Selected",
    removed: (count: number) => `Removed ${count} ${count === 1 ? "link" : "links"}`,
    start: "Start",
    downloadAgain: "Download Again",
    remove: "Remove",
    tryAgain: "Try Again",
    preset: "Preset",
    waiting: "Waiting to be read",
    reading: "Reading…",
    startsWhenRead: "Starts once it's read",
    startsWhenUnlocked: "Starts once it's unlocked",
    /** A link that lists several items, under Needs Attention: what it is, by its address, and what to do. */
    toPick: {
      channel: "This is a channel. Pick which of its videos to download.",
      playlist: "This is a playlist. Pick which of its videos to download.",
      account: "This is an account. Pick which of its posts to download.",
      /** Reading it said it lists several items, but its address doesn't say what it is. */
      other: "This link lists several items. Pick which to download.",
    },
    pick: "Pick What to Download",
    inQueue: "Already in Downloads.",
    overwriteTitle: "Download Again?",
    overwriteMessage: (count: number) =>
      count === 1
        ? "One of these was already downloaded with the same preset, so its new file will replace the one made before."
        : `${count} of these were already downloaded with the same preset, so their new files will replace the ones made before.`,
    overwriteConfirm: "Replace",
    groups: {
      /** Channels, playlists and accounts to pick from, above the rest whatever the grouping. */
      needsAttention: "Needs Attention",
      ready: "Ready to Start",
      downloadedBefore: "Downloaded Before",
      notRead: "Not Read Yet",
      needsPassword: "Needs a Password",
      failed: "Couldn't Be Read",
      inQueue: "In Downloads",
    },
    /** The heading over the links that don't need attention, while some do and the list isn't grouped. */
    otherLinks: "Other Links",
  },
  /** The preview card's list of a channel's, playlist's or account's items. */
  picker: {
    kinds: { channel: "Channel", playlist: "Playlist", account: "Account" },
    /** What a listing's items are called: posts on an account, videos on YouTube, items elsewhere. */
    nouns: {
      post: ["post", "posts"],
      video: ["video", "videos"],
      item: ["item", "items"],
    },
    /** How many it lists: "Newest 50 videos", "Newest 50 of 8,634 posts", "95 videos". */
    listed: (count: number, noun: readonly [string, string], newestOnly: boolean, total: number | null) => {
      if (!newestOnly) return `${count.toLocaleString()} ${count === 1 ? noun[0] : noun[1]}`
      const all = total !== null && total > count ? ` of ${total.toLocaleString()}` : ""
      return `Newest ${count.toLocaleString()}${all} ${(total ?? count) === 1 ? noun[0] : noun[1]}`
    },
    empty: "There's nothing to download here yet.",
    selectAll: "Select All",
    select: "Select",
    selected: (selected: number, count: number) => `${selected.toLocaleString()} of ${count.toLocaleString()} selected`,
    /** After the count, while Hide Downloaded or the type leaves some out. */
    hidden: (count: number) => `${count.toLocaleString()} hidden`,
    /** The type filter (ItemTypeFilter.tsx), offered while the items are of several types. */
    type: "Type",
    types: { all: "All", images: "Images", videos: "Videos", mixed: "Mixed", files: "Files" },
    /** A type with how many items are of it: "Images (32)". */
    typeCount: (type: string, count: number) => `${type} (${count.toLocaleString()})`,
    typeHints: {
      all: "Shows every type.",
      images: "Shows only posts of images.",
      videos: "Shows only videos, and posts of videos only.",
      mixed: "Shows only posts holding several kinds of files, such as images and videos.",
      files: "Shows only other files, such as archives.",
    },
    hideDownloaded: "Hide Downloaded",
    hideDownloadedHint: "Hides what's downloaded or already in Downloads.",
    allHidden: "Everything listed so far is downloaded or in Downloads.",
    loadMore: "Load More",
    loadMoreHint: (noun: readonly [string, string]) => `Lists the next ${noun[1]}, older than these.`,
    /** Lists on past what's downloaded until this many new ones are listed. */
    loadNew: (count: number) => `Load ${count} New`,
    loadNewHint: (count: number, noun: readonly [string, string]) =>
      `Lists on past the ${noun[1]} downloaded or in Downloads until ${count} that aren't are listed, or the last one is. Stop keeps what's listed by then.`,
    /** "Load All (8,634)", with how many the site says it holds, when it says. An X account says how many photos and
     * videos it holds (`media`), never how many posts: "Load All (372 Photos & Videos)". */
    loadAll: (total: number | null, media: number | null = null) => {
      if (total !== null) return `Load All (${total.toLocaleString()})`
      return media === null ? "Load All" : `Load All (${photosAndVideos(media, "label")})`
    },
    /** Without a total: whether it's being counted (a YouTube channel's videos), the count couldn't be read, or the
     * site doesn't say. On an X account (`media`), what the site counts instead: its photos and videos, `total` of
     * them, of which the posts listed so far hold `listed`. */
    loadAllHint: (
      count: number,
      total: number | null,
      noun: readonly [string, string],
      counted: "counting" | "failed" | null,
      media: { total: number; listed: number } | null = null,
    ) => {
      const keeps = "Stop keeps what's listed by then."
      if (total !== null) {
        return `Lists the other ${Math.max(total - count, 0).toLocaleString()} ${noun[1]}, a page at a time. ${keeps}`
      }
      if (media !== null) {
        // Posts listed that hold as many as the site counts, or more, say nothing about how far along the list is.
        const held =
          media.listed < media.total
            ? `: the ${count.toLocaleString()} ${count === 1 ? noun[0] : noun[1]} listed ${count === 1 ? "holds" : "hold"} ${media.listed.toLocaleString()} of them`
            : ""
        return `Lists everything, a page at a time. The site counts ${photosAndVideos(media.total, "sentence")} here, not ${noun[1]}${held}. Its count includes some it doesn't list, GIFs for one. ${keeps}`
      }
      const unknown = {
        counting: `Counting how many ${noun[1]} there are…`,
        failed: `How many ${noun[1]} there are couldn't be read.`,
        none: `The site doesn't say how many ${noun[1]} there are.`,
      }[counted ?? "none"]
      return `Lists everything, a page at a time. ${unknown} ${keeps}`
    },
    /** While Load New or Load All reads: "Listing… 150 of 8,634 posts · 12 new". On an X account (`media`), whose count
     * is of photos and videos: "Listing… 150 posts · 212 of 372 photos & videos · 12 new", without "of 372" once the
     * posts listed hold as many or more. */
    loadingAll: (
      count: number,
      total: number | null,
      noun: readonly [string, string],
      fresh: number,
      media: { total: number; listed: number } | null = null,
    ) => {
      const listing =
        total !== null && total > count
          ? `Listing… ${count.toLocaleString()} of ${total.toLocaleString()} ${noun[1]}`
          : `Listing… ${count.toLocaleString()} ${count === 1 ? noun[0] : noun[1]}`
      const held =
        total !== null || media === null
          ? []
          : [
              media.listed < media.total
                ? `${media.listed.toLocaleString()} of ${photosAndVideos(media.total, "details")}`
                : photosAndVideos(media.listed, "details"),
            ]
      return [listing, ...held, `${fresh.toLocaleString()} new`].join(" · ")
    },
    /** Under Load All's count on Instagram, which asks to slow down when read quickly. */
    loadingSlowly: "Instagram is read a page of 12 at a time, about 10 seconds apart, so it doesn't ask to slow down.",
    /** In place of the count while the site makes the read wait (X's limit): "Paused by X, resuming at 1:42 AM (12:31
     * left)". */
    pausedBySite: (site: string, time: string, left: string | null) =>
      `Paused by ${site}, resuming at ${time}${left ? ` (${left})` : ""}`,
    /** Under it: nothing to do, and how long the rest takes at most (`rest`, from `roughTime`) when the site's count
     * says. "At most", since X's lists may end long before its count: an old account's lists don't reach its oldest
     * posts. */
    pausedNote: (noun: readonly [string, string], rest: string | null) =>
      `It goes on by itself, and the ${noun[1]} listed can be picked meanwhile.${rest ? ` By X's count, the rest takes up to ${rest}.` : ""}`,
    /** Under Load All on an X account too big to list in one go: how X paces it, and how long it all takes at most. */
    paceBefore: (items: number, minutes: number, noun: readonly [string, string], time: string) =>
      `X lists about ${items.toLocaleString()} ${noun[1]}, then makes the next ones wait up to ${minutes} minutes. By its count, Load All takes up to ${time} here, going on by itself after each wait; it ends sooner when X lists fewer ${noun[1]} than it counts. What's listed is kept, so it can be stopped and picked up again later.`,
    /** Under the count while Load All lists such an account. */
    paceWhile: (items: number, minutes: number, noun: readonly [string, string], time: string) =>
      `X lists about ${items.toLocaleString()} ${noun[1]}, then makes the next ones wait up to ${minutes} minutes: up to ${time} left by its count.`,
    /** A stretch of time to the quarter of an hour, or to the hour from 4 hours: "45 minutes", "1 hour 15 minutes",
     * "7 hours". */
    roughTime: (minutes: number) => {
      const quarters = Math.max(Math.round(minutes / 15), 1)
      const hours = quarters >= 16 ? Math.round(quarters / 4) : Math.floor(quarters / 4)
      const rest = quarters >= 16 ? 0 : (quarters % 4) * 15
      const parts = [
        hours > 0 ? `${hours.toLocaleString()} ${hours === 1 ? "hour" : "hours"}` : "",
        rest > 0 ? `${rest} minutes` : "",
      ]
      return parts.filter(Boolean).join(" ")
    },
    stopLoading: "Stop",
    downloaded: "Downloaded",
    inQueue: "In Downloads",
    inQueueHint: "Already in Downloads.",
    videosStartAs: "Videos Start As",
    videosStartAsHint: "For videos no rule picks a preset for.",
    downloadSelected: (count: number) => (count === 0 ? "Download Selected" : `Download Selected (${count.toLocaleString()})`),
  },
  /** How a picker lays out what it offers: a listing's items, a post's files (PickerViewControls.tsx). */
  pickerView: {
    view: "View",
    views: { list: "List", masonry: "Masonry", gallery: "Gallery" },
    size: "Picture Size",
    smaller: "Smaller Pictures",
    larger: "Larger Pictures",
  },
  /** The Watched view: channels, playlists and accounts checked for what's new, and Watch on the picker. */
  watched: {
    watch: "Watch",
    watchTitle: "Watch This Source",
    watchHint: "Keeps it in Watched, where Check Now lists what's new on it.",
    firstItems: "What Counts as New Now",
    fromNow: "Nothing yet: only what's added from now on",
    latest: "Its latest",
    latestItems: (noun: string) => noun,
    all: (count: number, noun: string) => `Everything listed here (${count} ${noun})`,
    saveIn: "Save In",
    saveInHint: "Beats every rule. {site}, {uploader} and the other placeholders work. Empty lets the rules decide.",
    rulesDecide: "The Rules Decide",
    chooseFolder: "Choose Folder…",
    chooseFolderTitle: "Choose Where This Source's Downloads Go",
    clearFolder: "Let the Rules Decide",
    videosStartAs: "Videos Start As",
    rulesPreset: "As the Rules Say, Else the Default",
    cancel: "Cancel",
    added: "Watching it now. It's in Watched.",
    open: "Open Watched",
    emptyTitle: "Nothing Watched",
    emptyBody:
      "Paste a channel, playlist or account link, then Watch on its list. Watched sources stay here, and Check Now lists what's new on them.",
    checkAll: "Check All",
    checkAllHint: "Checks every source, one after the other",
    checkGroup: (group: string) => `Check All Under ${group}`,
    show: "Show",
    shows: {
      sources: "Sources",
      allNew: (count: number) => (count === 0 ? "All New Items" : `All New Items (${count})`),
    },
    groupBy: "Group By",
    groups: {
      none: "None",
      site: "Site",
    },
    sortBy: "Sort By",
    sorts: {
      mostNew: "Most New",
      name: "Name",
      lastChecked: "Last Checked",
      latestPost: "Latest Post",
      added: "Date Watched",
    },
    ascending: "Ascending",
    descending: "Descending",
    expandAll: "Expand All",
    collapseAll: "Collapse All",
    expand: (name: string) => `Expand ${name}`,
    collapse: (name: string) => `Collapse ${name}`,
    /** The group of the sources whose last check failed, above the rest. */
    needsAttention: "Needs Attention",
    /** The heading over the other sources, while some need attention and the list isn't grouped. */
    otherSources: "Other Sources",
    groupNew: (count: number) => `${count} new`,
    downloadedCount: (count: number) => `${count} downloaded`,
    skippedCount: (count: number) => `${count} skipped`,
    latestPost: (ago: string) => `Last post ${ago}`,
    showInHistory: "Show Downloads in History",
    latestDownloads: "Latest Downloads",
    openDownload: (title: string) => `Open ${title}`,
    downloadedOn: (day: string) => `Downloaded ${day}`,
    allNewNone: "Nothing new on any source. Check All looks for more.",
    /** A source in All New Items when another watched source has its name: "NASA on x.com". */
    sourceOnSite: (name: string, site: string) => `${name} on ${site}`,
    checkNow: "Check Now",
    checking: "Checking…",
    edit: "Edit",
    remove: "Stop Watching",
    removeTitle: "Stop Watching?",
    removeMessage: (name: string) =>
      `${name} leaves Watched, with what it found. Its downloads stay in Downloads and the history.`,
    removeConfirm: "Stop Watching",
    moreActions: "More Actions",
    checked: (ago: string) => `Checked ${ago}`,
    neverChecked: "Not checked yet",
    newCount: (count: number) => (count === 0 ? "Nothing new" : `${count} new`),
    savesIn: (folder: string) => `Saves in ${folder}`,
    savesByRules: "Saves where the rules say",
    startsAs: (preset: string) => `Videos start as ${preset}`,
    noneNew: "Nothing new since the last check.",
    selectAll: "Select All",
    selected: (selected: number, count: number) => `${selected} of ${count} selected`,
    downloadSelected: (count: number) => (count === 0 ? "Download Selected" : `Download Selected (${count.toLocaleString()})`),
    downloadSelectedHint:
      "The items shown that you leave unticked are skipped, and never come back as new. Those another type hides stay new.",
    skipAll: "Skip All",
    skipAllHint: "Skips every item shown. Those another type hides stay new.",
    skipped: (count: number) => `Skipped Before (${count})`,
    download: "Download",
    save: "Save",
    discard: "Discard",
    errors: {
      notFound: "This source isn't watched anymore.",
      alreadyWatched: "This source is watched already.",
      storage: "Couldn't save the watched sources.",
    },
  },
  /** The field that opens a link that needs a password, on the preview card, a pending link and a download. */
  password: {
    field: "Password",
    unlock: "Unlock",
  },
  preview: {
    fetching: "Fetching the details…",
    /** While a channel, playlist or account is listed, instead of `fetching`. */
    listing: {
      channel: "Listing the channel's newest videos…",
      playlist: "Listing the playlist's videos…",
      account: "Listing the account's newest posts… This can take a minute.",
    },
    cancel: "Cancel",
    dismiss: "Dismiss",
    close: "Close",
    download: "Download",
    downloadAgain: "Download Again",
    quality: (height: number) => `${height}p`,
    audioOnly: "Audio Only",
    postContents: (images: number, videos: number, others: number) => {
      const parts = [
        images > 0 && `${images} ${images === 1 ? "image" : "images"}`,
        videos > 0 && `${videos} ${videos === 1 ? "video" : "videos"}`,
        others > 0 && `${others} ${others === 1 ? "file" : "files"}`,
      ].filter(Boolean)
      return parts.join(", ")
    },
    /** A file host's link: one file's size, or a shared folder's file count and size. */
    fileContents: (count: number, size: string | null) =>
      size === null ? (count === 1 ? "Size unknown" : `${count} files`) : count === 1 ? size : `${count} files, ${size}`,
    alreadyDownloaded: (date: string, preset: string) =>
      preset ? `Already downloaded on ${date} with ${preset}.` : `Already downloaded on ${date}.`,
    openFile: "Open File",
    showInFolder: "Show in Folder",
    preset: "Preset",
    savesIn: (folder: string) => `Saves in ${folder}`,
    destinationFrom: (source: string) => `· ${source}`,
    destinationSources: {
      /** A rule's name, or "Rule 3" (`ruleLabel`). */
      rule: (rule: string) => rule,
      preset: "Preset's Folder",
      downloadFolder: "Download Folder",
      chosen: "Picked for This One",
      /** A rule picked for this one download (`ruleLabel`), which decides what it sets whatever its conditions. Short,
       * so the folder before it keeps its room. */
      pickedRule: (rule: string) => `Picked: ${rule}`,
    },
    pickedRuleHint: (rule: string) => `${rule} applies to this one, whatever its conditions`,
    chooseFolder: "Change Folder…",
    chooseFolderTitle: "Choose Where This Download Goes",
    usualFolder: "Use the Usual Folder",
    /** Picks a rule for this one download: its folder, preset and file name win over every other rule's. */
    applyRule: "Apply a Rule…",
    /** In Apply a Rule's list, a rule that's off, which still applies when picked. */
    ruleOff: (rule: string) => `${rule} (Off)`,
    matchingRules: "Use the Matching Rules",
    /** Under where a download goes, when the manual rule picked for it doesn't match it (it still applies). */
    pickedMismatch: {
      nothingCaptured: (rule: string) =>
        `${rule}'s conditions don't match this link. It still applies, but its pattern finds nothing here, so {1}, {2}… have no value.`,
      conditions: (rule: string) =>
        `${rule}'s conditions don't match this link. It still applies, since it was picked for it.`,
    },
    /** Beside the preset picker when a rule picked the preset, opening the rules. */
    presetByRule: (rule: string) => rule,
    presetByRuleHint: (rule: string) => `${rule} picks this preset`,
    inQueue: "This is already in Downloads.",
    overwriteTitle: "Download Again?",
    overwriteMessage: (preset: string) =>
      `It was already downloaded with ${preset}, so the new file will replace the one made before.`,
    overwriteConfirm: "Replace",
    missingDeno: "Deno isn't set up, so some YouTube formats may be missing.",
    setUpDeno: "Set Up Deno",
    errors: {
      invalidLink: "That isn't a link. Paste a link that starts with https://.",
      playlist: "This link lists several items. Paste it on its own to pick which to download.",
      notListable: (site: string) =>
        `This ${site || "site's"} page holds several items, which Tsundoku can't list yet. Paste a link to a single one instead.`,
      liveStream: "This is a live stream, which can't be downloaded yet.",
      upcoming: "This video hasn't started yet. Try again once it has.",
      unsupportedSite: "This site isn't supported.",
      noVideo: "There's nothing to download here.",
      imagesNotSetUp:
        "This looks like a post of images. Downloading images takes one more free tool, which Tsundoku can set up.",
      unavailable: "This isn't available. It may be private, removed or blocked in your country.",
      signInRequired: "This needs a signed-in account or an age check.",
      signInOff: (site: string | null) =>
        `${site ?? "This site"} needs you signed in. Turn on cookies so Tsundoku can use your browser's sign-in.`,
      signInNeeded: (site: string) => `${site} needs you signed in. Sign in to ${site} in your browser, then try again.`,
      signInRefused: (site: string) =>
        `${site} didn't accept your browser's sign-in. Signing in to ${site} again in your browser often fixes this.`,
      limitReached: (site: string) =>
        `${site}'s download limit is used up for now. Try again in a few hours.`,
      waitBetweenDownloads: (site: string, time: string) =>
        `${site} makes free downloads wait between each other. Try again after ${time}.`,
      noSupportedMirror: (site: string, mirrors: string[]) =>
        mirrors.length === 0
          ? `This ${site} page lists no mirrors.`
          : `None of this ${site} page's mirrors is on a site Tsundoku downloads from: ${mirrors.join(", ")}.`,
      shortLinkPage: (site: string) =>
        `This ${site} link shows a page before leading anywhere, so Tsundoku can't follow it. Open it in your browser, then copy the link it ends on and add that instead.`,
      shortLinkToUnsupportedSite: (site: string, target: string) =>
        `This ${site} link leads to ${target}, which Tsundoku can't download from.`,
      subscribersOnly: (site: string) => `The owner of this ${site} file keeps it for ${site}'s subscribers.`,
      passwordProtected: (site: string) =>
        `This ${site} link needs a password. Tsundoku can't enter passwords for ${site} links yet.`,
      passwordNeeded: (wrong: boolean, tried: number) =>
        wrong
          ? "That password didn't open this link. Check it and try again."
          : tried === 0
            ? "This link needs a password."
            : tried === 1
              ? "This link needs a password. The one Tsundoku tried didn't open it."
              : `This link needs a password. None of the ${tried} Tsundoku tried opened it.`,
      linkIncomplete: (site: string) =>
        `This ${site} link is missing its key, or it's cut off. Copy the whole link again.`,
      browserCheck: (site: string) =>
        `${site} wants a browser check before this download. Open the link in your browser, click Download, then copy the link of the download it starts and add that instead.`,
      nestedFolders: (site: string) =>
        `This ${site} folder has folders inside it, which Tsundoku can't read yet. Download it from your browser instead.`,
      linkNotRecognized: (site: string) =>
        `This ${site} link isn't one Tsundoku recognizes. Try the link to the file or folder itself.`,
      network: "Couldn't reach the site. Check your connection, then try again.",
      tooManyRequests: (site: string | null, time: string | null) =>
        `${site ?? "The site"} asked to slow down. ${time ? `Try again after ${time}.` : "Try again in a few minutes."}`,
      listingPaused: (site: string) =>
        `${site} lists only so many posts every 15 minutes, and that's used up for now. Try again in a few minutes.`,
      siteError: "Couldn't read this site's page. An update often fixes this.",
      failed: "Couldn't read this link.",
    },
  },
  presets: {
    defaultBadge: "Default",
    builtIn: "Built-In",
    setAsDefault: "Set as Default",
    duplicate: "Duplicate",
    delete: "Delete",
    reset: "Reset",
    copyName: (name: string) => `${name} (Copy)`,
    add: "Add Preset",
    newName: "New Preset",
    restoreBuiltIn: "Put Back a Built-In Preset",
    restoreItem: (name: string) => `Put Back ${name}`,
    moveUp: "Move Up",
    moveDown: "Move Down",
    saving: "Saving…",
    saved: "Saved.",
    summary: {
      video: (quality: string, container: string) => `Video · ${quality} · ${container}`,
      audio: (format: string) => `Audio · ${format}`,
    },
    fields: {
      name: "Name",
      type: "Type",
      video: "Video",
      audio: "Audio",
      quality: "Quality",
      best: "Best",
      upTo: (height: number) => `Up to ${height}p`,
      container: "Container",
      containers: { mkv: "MKV", mp4: "MP4", original: "Original" },
      audioFormat: "Format",
      audioFormats: { original: "Original", mp3: "MP3", m4a: "M4A", opus: "Opus", flac: "FLAC" },
      fileName: "File Name",
      folder: "Folder",
      chooseFolder: "Choose Folder…",
      chooseFolderTitle: "Choose Where Downloads Go",
      useDefaultFolder: "Use Default",
      embed: "Embed",
      metadata: "Metadata",
      thumbnail: "Thumbnail",
      chapters: "Chapters",
      subtitles: "Subtitles",
      subtitleLanguages: "Subtitle Languages",
      sponsorBlock: "SponsorBlock",
      sponsorBlockModes: { off: "Off", mark: "Mark as Chapters", remove: "Remove Sponsors" },
      extraArguments: "Extra Arguments",
      arguments: "yt-dlp Arguments",
    },
    hints: {
      containers: {
        mkv: "Holds any codec, so nothing is re-encoded.",
        mp4: "Plays everywhere. H.264 and AAC formats are preferred, so it's rarely re-encoded.",
        original: "Whatever yt-dlp merges into.",
      },
      audioFormats: {
        original: "The best audio as it is, usually Opus or M4A.",
        mp3: "Converted to MP3 at the best quality.",
        m4a: "M4A, converted only when the site has none.",
        opus: "Opus, converted only when the site has none.",
        flac: "Converted to FLAC.",
      },
      fileName: "yt-dlp's output template. The ID in the name lets Tsundoku recognize the file later.",
      defaultFolder: (path: string) => `The download folder, set in Settings: ${path}`,
      subtitleLanguages: "yt-dlp's language codes, separated by commas, such as en.*,ja.",
      extraArguments: "Added after Tsundoku's own, so they can change them.",
      arguments: "What Tsundoku passes to yt-dlp for this preset, besides its own progress and folder options.",
    },
    deleteTitle: "Delete Preset?",
    deleteMessage: (name: string) => `Delete ${name}? Downloads made with it stay in the history.`,
    discardTitle: "Discard Changes?",
    discardMessage: (name: string) => `${name} has changes that couldn't be saved. Discard them?`,
    discardConfirm: "Discard",
    errors: {
      emptyName: "Give the preset a name.",
      emptyFileName: "The file name can't be empty.",
      refusedArgument: (argument: string) =>
        `${argument} can't be used in a preset: Tsundoku relies on what it changes.`,
      unclosedQuote: "A quote in the extra arguments isn't closed.",
      notFound: "This preset doesn't exist anymore.",
      isDefault: "The default preset can't be deleted. Make another one the default first.",
      storage: "Couldn't save the presets.",
    },
  },
  rules: {
    intro:
      "Rules pick where downloads go, the preset videos start with, whether copied links download right away, and how archives are unpacked. For each, the last rule that matches and picks it decides, so a rule overrides those above it. A preset with a folder of its own comes before the first rule, which takes everything else.",
    /** The (?) at the top of the view: what each of the editor's fields means, under the field's own label. */
    help: {
      label: "How Rules Work",
      name: "Shown instead of the rule's number, such as “Rule 3”, wherever the rule is named, such as beside a download's folder. Leave it empty to use its number.",
      when: [
        "Conditions joined by OR are alternatives, of which one must match; AND joins those, and each must. So “site is X OR site is Bluesky AND title contains cat” takes a cat from either site. A rule without conditions matches every download.",
        "Text is compared ignoring case and the spaces around it, unless Match Case is on. Matches takes a regular expression, found anywhere in the text; what it captures in parentheses can go in the folder and the file name as {1}, {2} and so on.",
      ],
      saveIn: (downloadFolder: string) => [
        "Placeholders such as {site} become the download's own: click one to put it where the cursor is, and hover over it to see what it becomes.",
        `A folder that isn't a full path goes inside the download folder, ${downloadFolder}. Leave it empty to let a rule above pick the folder.`,
      ],
      fileName: [
        "How the files of matching posts are named, as in Settings → Posts of Images, and a matching shared file (from MEGA, Google Drive or another file host) when it's a single file. Videos and audio keep their preset's name.",
        "{1}, {2}… are what the rule's Matches pattern captured in parentheses. A post's name gets {post} and {number} added when it lacks them, so no two files share a name; the extension is added, or kept for a shared file. Leave it empty to let a rule above name them, else Settings or the file's own name.",
      ],
      missingValues: [
        "What a placeholder becomes when the download has no value for it, such as {account} on MediaFire, which has no accounts.",
        "Write Unknown puts Unknown in its place in the folder (No Playlist for {playlist}, Posts or Files for {preset}), and the site in a file name's {account}.",
        "Leave Out drops it with the separator before it, and a folder whose placeholders all have none, so {site}\\{account} saves in MediaFire. In a file name, {account} goes too. An unknown date or {1} is left out of a file name either way.",
      ],
      startAs: [
        "The preset matching videos start with, unless you pick another. Posts and shared files have none.",
        "A rule that checks the kind or the preset can't pick the preset, since both depend on it.",
      ],
      downloadRightAway:
        "Whether matching links you copy, or that the clipboard shortcut or your browser send, start downloading once they're read. Leave as Is follows Settings.",
      manual: [
        "Manual rules never take a download by themselves. Apply one to a link with Apply a Rule…, the rules button beside where it will be saved, on its card, in Pending or in Test a Link: it then decides the folder, file name, preset and unpacking it sets for that link, before every other rule and whatever its conditions.",
        "Their conditions are there for their patterns: what a Matches pattern captures in parentheses fills {1}, {2} and so on in the folder and the file name. On a link the pattern doesn't match, those have no value, as When a Placeholder Has No Value says.",
        "They're always on, and have no Download Right Away, since copied links are decided before a rule is applied. A rule's More Actions menu moves it to the other list.",
      ],
      unpackArchives: [
        "Whether the zip, 7z and rar archives that matching downloads bring are unpacked, and where. Default follows Settings → Archives. Skip leaves them as they're downloaded, even while Settings unpacks.",
        "Smart puts an archive's one file or folder beside it as it is, and several in a folder named after the archive. In Place puts everything beside the archive as it is, with no folder made for it. Folder puts everything in a folder named after the archive, even a single file.",
        "The passwords, and what becomes of the archive once unpacked, are Settings'.",
      ],
    },
    addRule: "Add Rule",
    /** The two lists of rules: those that take the downloads they match, and those only applied by hand. */
    sections: { automatic: "Automatic Rules", manual: "Manual Rules" },
    manualIntro:
      "Manual rules never pick downloads by themselves: apply one to a link with Apply a Rule…, beside where it will be saved. It then decides what it sets for that link, whatever its conditions, and what its patterns capture fills {1}, {2} and so on.",
    addManualRule: "Add Manual Rule",
    noManualRules: "No manual rules yet.",
    moveToManual: "Move to Manual Rules",
    moveToAutomatic: "Move to Automatic Rules",
    emptyTitle: "No Rules",
    emptyBody:
      "Add a rule to pick the folder, the preset, Download Right Away or unpacking for downloads by site, uploader, title, link, kind or preset.",
    otherwise: (folder: string) => `The download folder is ${folder}.`,
    changeDownloadFolder: "Change Download Folder",
    tester: {
      title: "Test a Link",
      placeholder: "Paste a Link to See Where It Goes",
      test: "Test",
      reading: "Reading the link…",
      savesIn: "Saves In",
      startsAs: "Starts As",
      copiedLinks: "Copied Links",
      byRule: (rule: string) => rule,
      defaultPreset: "The default preset",
      noPreset: "No preset: a post or shared files are kept as they are.",
      rightAway: "Download right away",
      waitInPending: "Wait in Pending",
      asSettingsSay: "As Settings → Adding Links says",
      archives: "Archives",
      leftPacked: "Left as they are",
      unpackedTo: (place: string) => `Unpacked: ${place}`,
      asArchiveSettingsSay: "As Settings → Archives says",
      listing: "This is a channel, playlist or account. Each item picked from it is matched on its own, so test one of its links instead.",
      nothingDownloaded: "Nothing is downloaded.",
    },
    number: (number: number) => `Rule ${number}`,
    name: "Name",
    off: "Off",
    edit: "Edit",
    duplicate: "Duplicate",
    dragToMove: "Drag to Move",
    moveUp: "Move Up",
    moveDown: "Move Down",
    moreActions: "More Actions",
    turnOn: "Turn On",
    turnOff: "Turn Off",
    delete: "Delete",
    save: "Save",
    discard: "Discard",
    when: "When",
    /** A manual rule's conditions, which only fill its placeholders, in place of When. */
    conditions: "Conditions",
    /** Between two conditions, in capitals as in the rules' sentences; a click switches it. */
    join: { all: "AND", any: "OR" },
    joinHint: {
      all: "Both sides must match. Click for OR.",
      any: "Either side is enough. Click for AND.",
    },
    addCondition: "Add Condition",
    removeCondition: "Remove Condition",
    /** The letter-case switch on a condition, named by what a click does. */
    matchCase: "Match Case",
    ignoreCase: "Ignore Case",
    field: "What to Check",
    operator: "Comparison",
    value: "Value",
    saveIn: "Save In",
    placeholderHints: {
      site: "The download's site, such as Instagram or YouTube.",
      uploader: "The uploader's name, such as National Geographic, or Unknown when there's none.",
      account: "The uploader's handle, such as natgeo, or their name on a site without handles.",
      playlist: "The playlist, channel or account you picked it from, or No Playlist.",
      title: "The download's title, or a shared file or folder's name, cut to 80 characters.",
      id: "The ID its site gives it, such as jNQXAC9IVRw.",
      kind: "Videos, Audio, Posts or Files.",
      preset: "The preset's name. Posts and shared files have none, so they get Posts or Files.",
      year: "The year it was uploaded, such as 2026, or Unknown.",
      month: "The month it was uploaded, from 01 to 12, or Unknown.",
      date: "The day it was uploaded, as 2026-11-06, or Unknown.",
      added: "The day you added it, as 2026-11-06.",
    },
    capturedHint: (group: string) =>
      `The text the pattern captured in its parentheses number ${group}, counting from the left, or Unknown.`,
    alwaysFirst: "Always First",
    alwaysFirstHint:
      "Takes every download no rule below it takes, unless its preset has a folder of its own. Every other rule overrides it, and new rules go at the bottom.",
    catchAllWhen: "Every download no rule below takes, unless its preset has a folder of its own.",
    chooseFolder: "Choose Folder…",
    chooseFolderTitle: "Choose Where Matching Downloads Go",
    fileName: "File Name",
    usingDefault: "Using Default",
    missingValues: "When a Placeholder Has No Value",
    missingValueChoices: { standIn: "Write Unknown", leaveOut: "Leave Out" },
    startAs: "Start As",
    startAsUnavailable: "A rule that checks the kind or the preset can't pick the preset, since both depend on it.",
    leaveAsIs: "Leave as Is",
    deletedPreset: "Deleted Preset",
    downloadRightAway: "Download Right Away",
    on: "On",
    unpackArchives: "Unpack Archives",
    /** A rule's own two choices, before the places Settings → Archives offers too (`unpackPlaceChoices`). */
    unpackChoices: { default: "Default", off: "Skip" },
    /** What a rule does besides picking a folder, under its folder in the list. */
    actions: {
      namesFiles: (name: string) => `Names files “${name}”`,
      leavesOut: "Leaves out placeholders without a value",
      startsAs: (preset: string) => `Starts as ${preset}`,
      presetDeleted: "Starts with the default preset, since the one it picked was deleted",
      rightAway: "Downloads right away",
      notRightAway: "Waits to be started",
      leavesArchivesPacked: "Leaves archives packed",
      unpacksArchives: (place: string) => `Unpacks archives: ${place}`,
    },
    fields: {
      site: "Site",
      uploader: "Uploader",
      title: "Title",
      link: "Link",
      kind: "Kind",
      preset: "Preset",
    },
    placeholders: {
      site: "Site Name",
      uploader: "Account or Channel",
      title: "Words in the Title",
      link: "Part of the Link",
      pattern: String.raw`Pattern, such as ^\[(.+?)\]`,
    },
    operators: {
      is: "Is",
      isNot: "Is Not",
      contains: "Contains",
      doesNotContain: "Doesn't Contain",
      matches: "Matches",
      doesNotMatch: "Doesn't Match",
    },
    kinds: {
      video: "Video",
      audio: "Audio",
      post: "Post of Images",
      file: "Shared File",
    },
    sentence: {
      every: "Every download",
      matchingCase: "(matching case)",
      everythingElse: "Everything else",
      when: (conditions: string) => `When ${conditions}`,
      /** A manual rule's conditions, which don't decide when it applies. */
      manual: (conditions: string) => `Conditions: ${conditions}`,
      fields: {
        site: "the site",
        uploader: "the uploader",
        title: "the title",
        link: "the link",
        preset: "the preset",
      },
      operators: {
        is: "is",
        isNot: "isn't",
        contains: "contains",
        doesNotContain: "doesn't contain",
        matches: "matches",
        doesNotMatch: "doesn't match",
      },
      kind: (isNot: boolean, kind: string) => (isNot ? `it isn't ${kind}` : `it's ${kind}`),
      kinds: {
        video: "a video",
        audio: "audio",
        post: "a post of images",
        file: "a shared file",
      },
      quoted: (value: string) => `“${value}”`,
    },
    deleteTitle: "Delete Rule?",
    deleteMessage: (rule: string) => `Delete ${rule}? Downloads added already keep their folder.`,
    errors: {
      emptyFolder: (rule: number) => `Rule ${rule} needs a folder.`,
      noAction: (rule: number) =>
        `Rule ${rule} doesn't do anything yet. Pick a folder, a file name, a preset to start with, Download Right Away or how to unpack archives.`,
      circularPreset: (rule: number) =>
        `Rule ${rule} checks the kind or the preset, so it can't pick the preset too. Remove that condition, or set Start As to Leave as Is.`,
      emptyValue: (rule: number) => `A condition of rule ${rule} is empty. Fill it in or remove it.`,
      invalidPattern: (rule: number) => `A pattern in rule ${rule} isn't a valid regular expression.`,
      fileName: (rule: number, problem: string) => `Rule ${rule}'s file name can't be used: ${problem}`,
      storage: "Couldn't save the rules.",
    },
  },
  tools: {
    names: {
      ytDlp: "yt-dlp",
      ffmpeg: "ffmpeg",
      deno: "Deno",
      galleryDl: "gallery-dl",
    },
    descriptions: {
      ytDlp: "Downloads videos and audio.",
      ffmpeg: "Merges video and audio, and converts formats. Comes with ffprobe.",
      deno: "Runs YouTube's player code for yt-dlp. Without it, some YouTube formats may be missing.",
      galleryDl: "Downloads posts of images, such as Instagram's and X's, and files on sites such as Gofile and Pixeldrain.",
    },
    /** Where each tool's official builds come from (the core's `catalog`). */
    hosts: {
      ytDlp: "GitHub",
      ffmpeg: "GitHub",
      deno: "GitHub",
      galleryDl: "Codeberg",
    },
    managed: "Tsundoku's Copy",
    external: "Your Copy",
    notSetUp: "Not set up.",
    stages: {
      connecting: "Looking for the latest release…",
      downloading: (received: string, total: string | null) =>
        total === null ? `Downloading… ${received}` : `Downloading… ${received} of ${total}`,
      verifying: "Checking the download…",
      installing: "Installing…",
      checking: "Making sure it runs…",
      updating: "Updating…",
      waiting: "Waiting for the downloads using it to finish…",
    },
    errors: {
      network: "Couldn't reach the download site. Check your connection, then try again.",
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
      inUse: (name: string) => `${name} stayed in use by downloads, so it wasn't replaced. Try again once they finish.`,
      notManaged: (name: string) => `Only Tsundoku's own copy of ${name} can be updated.`,
      storage: "Couldn't write the tool's files.",
    },
  },
  setup: {
    choicesTitle: "Where Things Go",
    choicesBody:
      "Before setting up its tools, Tsundoku asks where they and your downloads go, so nothing appears anywhere you didn't choose.",
    toolsFolder: "Tools",
    toolsFolderHint: "The programs Tsundoku downloads with, about 370 MB once set up.",
    toolsInDataFolder: "In Tsundoku's Data Folder",
    toolsFolderTitle: "Choose Where the Tools Go",
    toolsInAppData: "In AppData",
    toolsInAppDataDetail: (path: string, found: boolean) =>
      found
        ? `${path}, where an installed Tsundoku's tools already are: they're used as they are.`
        : `${path}, shared with an installed Tsundoku and kept on this PC.`,
    caches: "Tool Caches",
    cachesHint: "Sign-ins and site data the tools keep between downloads. You can change this later in Settings.",
    cachesInDataFolder: "In Tsundoku's Data Folder",
    cachesInUsualFolders: "In the Tools' Usual Folders",
    cachesUsualDetail: "Your user folder, where yt-dlp, gallery-dl and Deno keep them on their own.",
    downloadFolder: "Download Folder",
    downloadFolderHint:
      "Where downloads go, unless a rule or a preset picks another folder. You can change it later in Settings.",
    besideApp: "Next to Tsundoku",
    besideAppDetail: (path: string) => `${path}, which moves with Tsundoku to another drive.`,
    inDownloads: "In Your Downloads Folder",
    downloadFolderTitle: "Choose Where Downloads Go",
    chooseFolder: "Choose a Folder…",
    noFolderChosen: "Pick any folder.",
    back: "Back",
    title: "Set Up Your Tools",
    body: "Tsundoku downloads videos and image posts with four free tools. Use copies you already have, or let Tsundoku download its own and keep them up to date.",
    lookingForTools: "Looking for tools on this PC…",
    useThisCopy: "Use This Copy",
    foundDetail: (version: string, path: string) => `Version ${version} · ${path}`,
    downloadFresh: "Download Fresh",
    downloadDetail: (size: string | null, host: string) =>
      size === null
        ? `From ${host}. Tsundoku keeps it up to date.`
        : `${size} from ${host}. Tsundoku keeps it up to date.`,
    notFound: (size: string | null) =>
      size === null
        ? "Not found on this PC, so Tsundoku will download it."
        : `Not found on this PC, so Tsundoku will download it (${size}).`,
    ready: (version: string | null) => (version === null ? "Ready." : `Ready: version ${version}.`),
    continue: "Continue",
    downloadAndContinue: "Download and Continue",
    tryAgain: "Try Again",
    cancel: "Cancel",
    continueWithout: (name: string) => `Continue Without ${name}`,
    continueWithoutThem: "Continue Without Them",
  },
  settings: {
    tools: "Tools",
    loadingTools: "Checking the tools…",
    updateNow: "Update Now",
    updatesDaily: "Tsundoku updates it once a day.",
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
    updateAll: "Update All",
    updateAllHint: "Brings every tool Tsundoku keeps its own copy of up to its latest version. Copies you chose yourself are left as they are.",
    updatingAll: "Updating the tools…",
    nothingToUpdate: "None of the tools is Tsundoku's own copy, so there's nothing to update.",
    allUpToDate: "Every tool is up to date.",
    updatedTools: (names: string[], upToDate: number) =>
      `Updated ${listOf(names)}.` + (upToDate === 0 ? "" : upToDate === 1 ? " The other one is up to date." : " The others are up to date."),
    channel: "Update Channel",
    channels: {
      stable: "Stable",
      nightly: "Nightly",
    },
    channelHint: "Nightly gets fixes for sites sooner, but is tested less.",
    caches: "Caches",
    cachesHint: "Where the tools keep sign-ins and site data between downloads, from their next run on.",
    cacheLocations: {
      dataFolder: "Data Folder",
      usualFolders: "Usual Folders",
    },
    usualCacheFolders: "Each tool's own folder in your user folder.",
    downloads: "Downloads",
    adding: "Adding Links",
    watchClipboard: "Watch Clipboard",
    watchClipboardHint:
      "Links you copy in any app go to Pending while Tsundoku is open, even minimized or in the tray. Copies holding anything besides links are left alone, and so are links to sites Tsundoku can't download from and links already downloaded or in Downloads.",
    clipboardShortcut: "Clipboard Shortcut",
    clipboardShortcutHint:
      "Press it in any app to add the links you copied to Pending, as if you'd pasted them there. Works without Watch Clipboard.",
    noShortcut: "None",
    /** The keys of each clipboard shortcut, as the core's ClipboardShortcut::keys names them. */
    shortcutKeys: {
      ctrlAltD: "Ctrl+Alt+D",
      ctrlShiftD: "Ctrl+Shift+D",
      ctrlAltV: "Ctrl+Alt+V",
      altShiftD: "Alt+Shift+D",
    },
    downloadRightAway: "Download Right Away",
    downloadRightAwayHint:
      "Links the clipboard watcher, the shortcut or your browser adds start downloading once they're read, with the default preset or the one a rule picks, instead of waiting in Pending. A rule can turn this on or off for the links it matches. A link that needs something from you first, such as signing in, still waits there, and so does one downloaded before.",
    browserBookmark: "Send from Your Browser",
    browserBookmarkHint:
      "A bookmark that sends the page you're on to Pending in one click. Copy its address, add a new bookmark to your browser's toolbar (name it Send to Tsundoku, say) and paste the address as its URL. The first time, your browser asks whether to open Tsundoku.",
    copyBookmark: "Copy Bookmark Address",
    bookmarkCopied: "Copied. Paste it as the new bookmark's URL.",
    sendButtons: "Send Buttons on Instagram, X and YouTube",
    sendButtonsHint:
      "A Send to Tsundoku button on each Instagram and X post, each YouTube video and Short, and YouTube's thumbnails, that sends it to Pending in one click. It needs the Tampermonkey or Violentmonkey add-on in your browser.",
    copyUserscript: "Copy Userscript",
    userscriptHowTo:
      "Open Tampermonkey or Violentmonkey, create a new script, paste the userscript over everything in it, then save.",
    userscriptCopied: "Copied. Paste it into a new script.",
    clickNLoad: "Take Links from Click'n'Load Buttons",
    clickNLoadHint: (sites: string[]) =>
      `Pages on ${listOf(sites)} can send a folder's links here with their Click'n'Load button, once you've passed their check in your browser. Tsundoku listens on this PC only, and the links wait in Pending to be started. JDownloader uses the same port, so only one of them can take the links at a time.`,
    clickNLoadPortTaken: (port: number) =>
      `Another app is already listening on port ${port}, where Click'n'Load buttons send their links. Close it (JDownloader, say), then try again.`,
    clickNLoadFailed: "Tsundoku couldn't start listening for Click'n'Load buttons.",
    tryAgain: "Try Again",
    notify: "Show Notifications",
    notifyHint:
      "While Tsundoku's window isn't in front: when a copied link is added or starts downloading, and when the shortcut or your browser adds links. Clicking one opens Tsundoku.",
    notifyFailures: "Notify When a Download Fails",
    notifyFailuresHint:
      "While Tsundoku's window isn't in front, with the download's title. Downloads that fail together are one notification.",
    notifyFinished: "Notify When Downloads Finish",
    notifyFinishedHint:
      "While Tsundoku's window isn't in front: once every download started has ended, with how many finished and how many failed.",
    background: "Running in the Background",
    /** Its name in the sidebar's list of sections, where the full one doesn't fit. */
    backgroundShort: "In the Background",
    keepInTray: "Keep Running in the Tray",
    keepInTrayHint:
      "Closing the window leaves Tsundoku in the notification area, still downloading and watching the clipboard. Click its icon to open it, or right-click for Watch Clipboard, Start All Pending and Quit.",
    startWithWindows: "Start with Windows",
    startWithWindowsHint:
      "Opens Tsundoku when you sign in to Windows. With Keep Running in the Tray on, it starts there, without its window.",
    appearance: "Appearance",
    theme: "Theme",
    themes: {
      auto: "Auto",
      light: "Light",
      dark: "Dark",
    },
    themeHint: "Auto follows Windows' light or dark mode.",
    accent: "Accent Color",
    accentHint: "For main buttons, selections, progress bars and links.",
    accents: {
      indigo: "Indigo",
      teal: "Teal",
      green: "Green",
      amber: "Amber",
      rose: "Rose",
      purple: "Purple",
      custom: "Pick a Color",
    },
    keyboardShortcuts: "Keyboard Shortcuts",
    shortcutActions: {
      paste:
        "Adds the links you copied, as Paste Link does, from anywhere but a text field: one shows its preview, several go to Pending.",
      typeLink: "Opens the field to type or edit a link, with its text selected.",
      views: (names: string[]) => `Open ${names.slice(0, -1).join(", ")} or ${names.at(-1)}, in the sidebar's order.`,
      delete: "Removes the ticked links in Pending.",
      clipboardShortcut: "Adds the links you copied to Pending from any app (Adding Links).",
    },
    folder: "Download Folder",
    folderHint: "Where downloads go, unless a rule or their preset picks another folder.",
    folderTitle: "Choose Where Downloads Go",
    atOnce: "Downloads at Once",
    atOnceHint: "How many downloads run at the same time.",
    rateLimit: "Speed Limit",
    rateLimitPlaceholder: "No Limit",
    rateLimitHint: "For each download, in bytes per second, such as 500K or 2M. Leave it empty for no limit.",
    defaultPreset: "Default Preset",
    defaultPresetHint: "Picked on the preview card unless you choose another.",
    posts: "Posts of Images",
    postFileName: "File Name",
    postFileNameHint:
      "How each file of a post is named, before its extension. It needs {post} and {number} (or {numberIfMany}, the number only when there are several files), so every file gets a name of its own. Files downloaded before keep their names.",
    postFileNameExample: "For example:",
    useDefaultName: "Use Default",
    postNamePlaceholderHints: {
      account: "The account that posted, such as natgeo.",
      date: "The day it was posted, such as 2026-11-04.",
      /** A rule's file name shows it too, for a shared file's ID. */
      post: "The ID its site gives it, such as DfPQchkG1tg.",
      number: "The file's place in the post: 1 for the first.",
      numberIfMany:
        "The file's place in the post, only when several of its files are downloaded: a post of one file, or one file picked from it, gets none.",
      site: "The site, such as Instagram or X.",
    },
    /** What the example under the field fills the placeholders with, and the extension it adds. */
    postNameSamples: {
      account: "natgeo",
      date: "2026-11-04",
      post: "DfPQchkG1tg",
      number: "1",
      numberIfMany: "1",
      site: "Instagram",
    },
    postNameSampleExtension: "jpg",
    postNameErrors: {
      missingPlaceholder: (placeholder: string) =>
        `The name needs {${placeholder}}, so no two files get the same name.`,
      unknownPlaceholder: (name: string) =>
        `{${name}} isn't a placeholder. The buttons under the field insert the ones there are.`,
      forbiddenCharacter: (character: string) => `A file name can't contain ${character}.`,
    },
    chooseFolder: "Choose Folder…",
    useDefaultFolder: "Use Default",
    defaultFolder: (path: string) => `The default: ${path}`,
    archives: "Archives",
    unpackArchives: "Unpack Archives After Downloading",
    unpackArchivesHint:
      "Zip, 7z and rar archives, in one file or split into parts, are unpacked beside them. An archive split over several links is unpacked once its last part is downloaded. A rule can say otherwise for the downloads it matches.",
    unpackPlace: "Where to Unpack",
    /** The places as the toggles name them, in Settings → Archives and a rule's editor. */
    unpackPlaceChoices: { smart: "Smart", inPlace: "In Place", archiveFolder: "Folder" },
    /** The places in full, where a sentence names one: a rule's line, Test a Link. */
    unpackPlaces: {
      smart: "Smart",
      archiveFolder: "In a Folder Named After the Archive",
      inPlace: "In Place",
    },
    afterUnpacking: "After Unpacking",
    afterUnpackingChoices: { recycleBin: "Recycle Bin", delete: "Delete", keep: "Keep" },
    /** The (?) beside Archives: what Where to Unpack's and After Unpacking's choices do. */
    archivesHelp: {
      label: "How Unpacking Works",
      unpackPlace: [
        "Smart puts an archive's one file or folder beside it as it is, and several in a folder named after the archive, so there's never a folder in a folder or a pile of loose files.",
        "In Place puts everything beside the archive as it is, with no folder made for it. Folder puts everything in a folder named after the archive, even a single file.",
        "A name already taken gets “(2)”, so nothing is replaced. A rule can pick another way, or none, for the downloads it matches.",
      ],
      afterUnpacking: [
        "What happens to the archive, all its parts, once what it holds is beside it. Recycle Bin moves them there, where they can be put back from; Delete deletes them for good; Keep leaves them where they are.",
        "An archive that couldn't be unpacked is always kept, so Retry unpacks it without downloading again.",
      ],
    },
    /** In Archives, where the passwords were before 0.2.6. */
    archivePasswordsElsewhere: "An archive that has a password is opened with the ones in Passwords, below.",
    passwords: "Passwords",
    passwordList: "Your Passwords",
    passwordListHint:
      "Tried in order on a link or an archive that needs a password, until one opens it. Type one and press Enter to add it. They're kept in Tsundoku's settings as they are, not hidden.",
    passwordListPlaceholder: "Add a Password",
    removePassword: (password: string) => `Remove ${password}`,
    passwordListSaved: (count: number) =>
      count === 0 ? "Saved: no passwords." : count === 1 ? "Saved: 1 password." : `Saved: ${count} passwords.`,
    savePastPasswords: "Save Past Passwords",
    savePastPasswordsHint:
      "A password you enter that opens a link or an archive is kept, apart from yours above, and tried after them the next time one is needed.",
    pastPasswordsKept: (count: number) => (count === 1 ? "1 past password is kept." : `${count} past passwords are kept.`),
    forgetPastPasswords: "Forget Past Passwords",
    forgetPastPasswordsQuestion: (count: number) =>
      count === 1
        ? "Forget the past password? Your own passwords above stay, and so does the one each download was opened with."
        : `Forget the ${count} past passwords? Your own passwords above stay, and so does the one each download was opened with.`,
    forgetPastPasswordsFailed: "Couldn't forget the past passwords.",
    /** The (?) beside Passwords: what's tried on a link or an archive that needs a password, and what's kept. */
    passwordsHelp: {
      label: "How Passwords Are Tried",
      order: "What's Tried",
      orderParagraphs: [
        "When a link or an archive needs a password, Tsundoku tries your passwords first, in their order, then the past ones, and asks you only when none of them opens it.",
        "A link is tried with at most 20 passwords, and only with the past ones of its own site, since trying a password sends it to that site. An archive is opened on this PC, so every past password is tried on it, and so is the one that opened its link.",
      ],
      past: "Past Passwords",
      pastParagraphs: [
        "While Save Past Passwords is on, a password you enter on a link, a download or an archive is kept once it has opened it, with the site it came from. Past passwords never join your own list.",
        "Turning it off stops keeping and trying them, and leaves the ones kept until you forget them. They're kept in Tsundoku's data as they are, not hidden, and never written to a download's log or Copy Details.",
      ],
    },
    pacing: "Pacing",
    pacingIntro: "Some sites block accounts that read too fast, so Tsundoku goes easy on these.",
    pacedSites: { instagram: "Instagram", x: "X", pixiv: "Pixiv", tumblr: "Tumblr" },
    /** One line per site, around its fields: "X from [0] to [2] seconds, [1] at once". */
    pacingLine: { from: "from", to: "to", seconds: "seconds,", atOnce: "at once" },
    pauseFrom: (site: string) => `${site}: Shortest Pause`,
    pauseTo: (site: string) => `${site}: Longest Pause`,
    siteAtOnce: (site: string) => `${site}: Downloads at Once`,
    /** Around the hourly limit's field: "At most [No Limit] Instagram files an hour". */
    hourlyLimitLine: { before: "At most", after: "Instagram files an hour" },
    hourlyLimit: "Hourly Limit",
    hourlyLimitPlaceholder: "No Limit",
    pacingHelp: {
      label: "How Pacing Works",
      pause: "Pause",
      pauseParagraphs: (longest: number) => [
        "Before each file of a post, and between the site's links read in Pending, Tsundoku waits a random time in the site's range, up to " +
          `${longest} seconds. 0 to 0 is no pause.`,
        "Instagram starts with 3 to 5 seconds, the others with 0 to 2.",
      ],
      atOnce: "At Once",
      atOnceParagraphs: [
        "How many of the site's downloads run at the same time, one to start with. The site's other downloads wait their turn while downloads from other sites go ahead.",
        "Each download running pauses on its own, so two at once read about twice as fast as one. Downloads at Once, in Downloads, still limits every download together.",
      ],
      hourlyLimitParagraphs: [
        "Leave it empty for no limit. Once the past hour holds this many Instagram files, the next Instagram download waits, then continues by itself.",
      ],
    },
    cookies: "Cookies",
    cookiesBrowser: "Read Cookies from",
    cookiesNone: "None",
    cookiesHint:
      "For links that need you signed in or old enough: Tsundoku reads your browser's cookies, as if you were browsing there. Automatic picks the browser profile signed in to each site.",
    chromiumWarning:
      "Chrome, Edge and other Chromium browsers lock their cookies on Windows while they're open, and often encrypt them, so they often can't be read. Firefox and Zen work.",
    browsers: {
      automatic: "Automatic",
      firefox: "Firefox",
      zen: "Zen",
      libreWolf: "LibreWolf",
      floorp: "Floorp",
      waterfox: "Waterfox",
      chrome: "Chrome",
      edge: "Edge",
      brave: "Brave",
      opera: "Opera",
      vivaldi: "Vivaldi",
      chromium: "Chromium",
    },
    saved: "Saved.",
    sites: {
      title: "Supported Sites",
      intro:
        "Every site Tsundoku can download from. Sites that need an account show which browser profiles are signed in to them; Tsundoku uses the most recent one.",
      loading: "Listing the supported sites…",
      search: "Search Sites",
      signedIn: "Signed In",
      notSignedIn: "Not Signed In",
      allSites: (count: number) => `All Sites (${count.toLocaleString()})`,
      noMatches: "No sites match.",
      profile: (browser: string, profile: string) => `${browser} · ${profile}`,
      signInHint: (site: string) => `Sign in to ${site} in a browser`,
      imagesNotSetUp: "Image sites such as Pixiv and DeviantArt are listed once image downloads are set up in Tools.",
    },
    about: "About",
    dataFolder: "Data Folder",
    dataFolderHint: "Settings, the history, the tools and each download's log.",
    openDataFolder: "Open Folder",
    dataFolderOpenFailed: "Couldn't open the data folder.",
    openDownloadFolder: "Open Folder",
    downloadFolderOpenFailed: "Couldn't open the download folder.",
    backup: "Back Up and Restore",
    backupHint:
      "Your settings, presets, folder rules and history in one file, to keep them safe or move them to another PC. The tools and logs aren't in it.",
    backUp: "Back Up…",
    restore: "Restore…",
    /** The name a new backup is offered, with today's date as 2026-11-06. */
    backupFileName: (date: string) => `Tsundoku Backup ${date}.zip`,
    backupDialogTitle: "Save a Backup",
    restoreDialogTitle: "Choose a Backup to Restore",
    backupFilter: "Tsundoku Backups",
    backupSaved: "Backup saved.",
    restoreTitle: "Restore This Backup?",
    restoreMessage:
      "Your settings, presets, folder rules and history will be replaced by the backup's. They're saved first as a backup in the data folder's backups folder, so restoring that undoes this. Tsundoku then restarts: downloads running now stop, and can be resumed.",
    restoreConfirm: "Restore",
    backupErrors: {
      write: "Couldn't save the backup.",
      read: "Couldn't open this file.",
      notABackup: "This file isn't a Tsundoku backup, or it's damaged.",
      newerVersion: (version: string) =>
        `This backup was made by Tsundoku ${version}, which is newer than this one. Update Tsundoku, then restore it.`,
      storage: "Couldn't restore the backup, so nothing was changed.",
    },
  },
} as const

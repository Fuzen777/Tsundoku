import type { BackupError } from "./bindings/BackupError"
import type { DownloadError } from "./bindings/DownloadError"
import type { HistoryError } from "./bindings/HistoryError"
import type { LinkFileError } from "./bindings/LinkFileError"
import type { PostNameError } from "./bindings/PostNameError"
import type { PresetError } from "./bindings/PresetError"
import type { PreviewError } from "./bindings/PreviewError"
import type { RuleError } from "./bindings/RuleError"
import type { SignInCookies } from "./bindings/SignInCookies"
import type { ToolError } from "./bindings/ToolError"
import type { ToolId } from "./bindings/ToolId"
import type { UnpackProblem } from "./bindings/UnpackProblem"
import type { WatchedError } from "./bindings/WatchedError"
import { formatBytes, formatClockTime } from "./format"
import { strings } from "./strings"

/*
  The core's errors are tagged unions (ToolError, PreviewError, HistoryError, PresetError, RuleError, DownloadError): an action that fails rejects with one.
  These turn each kind into a sentence from strings.ts, with the technical detail underneath.
*/

export interface ErrorDescription {
  message: string
  /** The technical cause, shown smaller under the message. */
  detail?: string
}

/** Whether an action's error came from the core, rather than from somewhere unexpected. */
function isCoreError<CoreError extends { kind: string }>(error: unknown): error is CoreError {
  return typeof error === "object" && error !== null && "kind" in error
}

function unexpected(error: unknown): ErrorDescription {
  return { message: strings.errors.unexpected, detail: String(error) }
}

/** Cancelling is the user's choice, so it gets no error message. */
export function isCancelled(error: unknown): boolean {
  return isCoreError(error) && error.kind === "cancelled"
}

export function describeToolError(error: unknown, id: ToolId): ErrorDescription {
  if (!isCoreError<ToolError>(error)) return unexpected(error)
  const name = strings.tools.names[id]
  const text = strings.tools.errors
  switch (error.kind) {
    case "network":
      return { message: text.network, detail: error.detail }
    case "rateLimited":
      return { message: text.rateLimited }
    case "missingFromRelease":
      return { message: text.missingFromRelease(error.file) }
    case "checksumMissing":
      return { message: text.checksumMissing(error.file) }
    case "checksumMismatch":
      return { message: text.checksumMismatch }
    case "blocked":
      return { message: text.blocked(name) }
    case "fileMissing":
      return { message: text.fileMissing, detail: error.path }
    case "ffprobeMissing":
      return { message: text.ffprobeMissing }
    case "notThisTool":
      return { message: text.notThisTool(name), detail: error.output || undefined }
    case "wontRun":
      return { message: text.wontRun(name), detail: error.detail }
    case "updateFailed":
      return { message: text.updateFailed(name), detail: error.detail || undefined }
    case "busy":
      return { message: text.busy(name) }
    case "inUse":
      return { message: text.inUse(name) }
    case "cancelled":
      return { message: strings.errors.cancelled }
    case "notManaged":
      return { message: text.notManaged(name) }
    case "storage":
      return { message: text.storage, detail: error.detail }
    case "unexpected":
      return { message: strings.errors.unexpected, detail: error.detail }
  }
}

/** Whether a link's error asks for its password, which the preview card, Pending and the queue ask for. */
export function needsPassword(error: unknown): error is Extract<PreviewError, { kind: "passwordNeeded" }> {
  return isCoreError<PreviewError>(error) && error.kind === "passwordNeeded"
}

export function describePreviewError(error: unknown): ErrorDescription {
  if (!isCoreError<PreviewError>(error)) return unexpected(error)
  const text = strings.preview.errors
  switch (error.kind) {
    case "invalidLink":
      return { message: text.invalidLink }
    case "playlist":
      return { message: text.playlist }
    case "notListable":
      return { message: text.notListable(error.site) }
    case "liveStream":
      return { message: text.liveStream }
    case "upcoming":
      return { message: text.upcoming, detail: error.detail || undefined }
    case "unsupportedSite":
      return { message: text.unsupportedSite }
    case "noVideo":
      return { message: text.noVideo, detail: error.detail }
    case "imagesNotSetUp":
      return { message: text.imagesNotSetUp }
    case "unavailable":
      return { message: text.unavailable, detail: error.detail }
    case "signInRequired":
      return { message: describeSignIn(error.site, error.cookies), detail: error.detail }
    case "limitReached":
      return { message: text.limitReached(error.site), detail: error.detail }
    case "waitBetweenDownloads":
      return { message: text.waitBetweenDownloads(error.site, formatClockTime(Date.now() + error.seconds * 1000)) }
    case "noSupportedMirror":
      return { message: text.noSupportedMirror(error.site, error.mirrors) }
    case "shortLinkPage":
      return { message: text.shortLinkPage(error.site) }
    case "shortLinkToUnsupportedSite":
      return { message: text.shortLinkToUnsupportedSite(error.site, error.target) }
    case "subscribersOnly":
      return { message: text.subscribersOnly(error.site) }
    case "passwordProtected":
      return { message: text.passwordProtected(error.site) }
    case "passwordNeeded":
      return { message: text.passwordNeeded(error.wrong) }
    case "linkIncomplete":
      return { message: text.linkIncomplete(error.site) }
    case "browserCheck":
      return { message: text.browserCheck(error.site) }
    case "linkNotRecognized":
      return { message: text.linkNotRecognized(error.site) }
    case "nestedFolders":
      return { message: text.nestedFolders(error.site) }
    case "network":
      return { message: text.network, detail: error.detail }
    case "tooManyRequests":
      return {
        message: text.tooManyRequests(error.site, error.until === null ? null : formatClockTime(error.until)),
        detail: error.detail || undefined,
      }
    case "siteError":
      return { message: text.siteError, detail: error.detail }
    case "failed":
      return { message: text.failed, detail: error.detail }
    case "tool":
      return describeToolError(error.error, error.tool)
    case "cancelled":
      return { message: strings.errors.cancelled }
  }
}

/** The sign-in hint: what to do depends on whether cookies are on and a browser is signed in to the site. */
function describeSignIn(site: string | null, cookies: SignInCookies): string {
  const text = strings.preview.errors
  if (cookies === "off") return text.signInOff(site)
  if (site === null || cookies === "unknown") return text.signInRequired
  return cookies === "notSignedIn" ? text.signInNeeded(site) : text.signInRefused(site)
}

export function describeHistoryError(error: unknown): ErrorDescription {
  if (!isCoreError<HistoryError>(error)) return unexpected(error)
  const text = strings.history.errors
  switch (error.kind) {
    case "database":
      return { message: text.database, detail: error.detail }
    case "newerVersion":
      return { message: text.newerVersion }
    case "notFound":
      return { message: text.notFound }
    case "fileMissing":
      return { message: text.fileMissing, detail: error.path }
    case "archive":
      return { message: text.archive, detail: error.detail }
    case "openFailed":
      return { message: text.openFailed, detail: error.detail }
  }
}

export function describePresetError(error: unknown): ErrorDescription {
  if (!isCoreError<PresetError>(error)) return unexpected(error)
  const text = strings.presets.errors
  switch (error.kind) {
    case "emptyName":
      return { message: text.emptyName }
    case "emptyFileName":
      return { message: text.emptyFileName }
    case "refusedArgument":
      return { message: text.refusedArgument(error.argument) }
    case "unclosedQuote":
      return { message: text.unclosedQuote }
    case "notFound":
      return { message: text.notFound }
    case "isDefault":
      return { message: text.isDefault }
    case "storage":
      return { message: text.storage, detail: error.detail }
  }
}

export function describeRuleError(error: unknown): ErrorDescription {
  if (!isCoreError<RuleError>(error)) return unexpected(error)
  const text = strings.rules.errors
  switch (error.kind) {
    case "emptyFolder":
      return { message: text.emptyFolder(error.rule) }
    case "noAction":
      return { message: text.noAction(error.rule) }
    case "circularPreset":
      return { message: text.circularPreset(error.rule) }
    case "emptyValue":
      return { message: text.emptyValue(error.rule) }
    case "invalidPattern":
      return { message: text.invalidPattern(error.rule), detail: error.detail }
    case "fileName":
      return { message: text.fileName(error.rule, describePostNameError(error.error).message) }
    case "storage":
      return { message: text.storage, detail: error.detail }
  }
}

export function describeDownloadError(error: unknown): ErrorDescription {
  if (!isCoreError<DownloadError>(error)) return unexpected(error)
  const text = strings.queue.errors
  switch (error.kind) {
    case "ytDlp":
      return describePreviewError(error.error)
    case "folder":
      return { message: text.folder, detail: `${error.path}: ${error.detail}` }
    case "noFile":
      return { message: text.noFile }
    case "preset":
      return describePresetError(error.error)
    case "alreadyQueued":
      return { message: text.alreadyQueued }
    case "notFound":
      return { message: text.notFound }
    case "damaged":
      return { message: text.damaged(error.file) }
    case "windowDownloadStopped":
      return { message: text.windowDownloadStopped(error.site) }
    case "wrongState":
      return { message: text.wrongState }
    case "storage":
      return { message: text.storage, detail: error.detail }
    case "invalidRateLimit":
      return { message: text.invalidRateLimit, detail: error.value }
    case "postFileName":
      return describePostNameError(error.error)
    case "nothingPicked":
      return { message: text.nothingPicked }
    case "shortcutTaken":
      return { message: text.shortcutTaken(error.keys) }
    case "unpack":
      return describeUnpackProblem(error.archive, error.problem)
    case "unexpected":
      return { message: strings.errors.unexpected, detail: error.detail }
  }
}

/** Why an archive a download brought wasn't unpacked; its files are kept, and Retry only unpacks. */
function describeUnpackProblem(archive: string, problem: UnpackProblem): ErrorDescription {
  const text = strings.queue.errors.unpack
  switch (problem.kind) {
    case "needsPassword":
      return { message: text.needsPassword(archive) }
    case "wrongPassword":
      return { message: text.wrongPassword(archive, problem.tried) }
    case "missingPart":
      return { message: text.missingPart(archive, problem.part) }
    case "damaged":
      return { message: text.damaged(archive), detail: problem.detail }
    case "unsupported":
      return { message: text.unsupported(archive), detail: problem.detail }
    case "noRoom":
      return { message: text.noRoom(archive, problem.drive, formatBytes(problem.needed), formatBytes(problem.free)) }
    case "write":
      return { message: text.write(archive), detail: `${problem.path}: ${problem.detail}` }
  }
}

export function describeWatchedError(error: unknown): ErrorDescription {
  if (!isCoreError<WatchedError>(error)) return unexpected(error)
  const text = strings.watched.errors
  switch (error.kind) {
    case "notFound":
      return { message: text.notFound }
    case "alreadyWatched":
      return { message: text.alreadyWatched }
    case "queue":
      return describeDownloadError(error.error)
    case "storage":
      return { message: text.storage, detail: error.detail }
  }
}

export function describeBackupError(error: unknown): ErrorDescription {
  if (!isCoreError<BackupError>(error)) return unexpected(error)
  const text = strings.settings.backupErrors
  switch (error.kind) {
    case "write":
      return { message: text.write, detail: error.detail }
    case "read":
      return { message: text.read, detail: error.detail }
    case "notABackup":
      return { message: text.notABackup, detail: error.detail }
    case "newerVersion":
      return { message: text.newerVersion(error.version) }
    case "storage":
      return { message: text.storage, detail: error.detail }
  }
}

/** Why a dropped file gave no links; anything else is the file failing to be read, such as a folder dropped. */
export function describeLinkFileError(error: unknown, name: string): ErrorDescription {
  const text = strings.addBar.files
  if (!isCoreError<LinkFileError>(error)) return { message: text.unreadable(name), detail: String(error) }
  switch (error.kind) {
    case "unsupportedType":
      return { message: text.unsupportedType, detail: error.name }
    case "tooLarge":
      return { message: text.tooLarge(error.name, formatBytes(error.maxBytes)) }
    case "noLinks":
      return { message: text.noLinks(error.name) }
    case "unreadable":
      return { message: text.unreadable(error.name), detail: error.detail }
  }
}

export function describePostNameError(error: PostNameError): ErrorDescription {
  const text = strings.settings.postNameErrors
  switch (error.kind) {
    case "missingPlaceholder":
      return { message: text.missingPlaceholder(error.placeholder) }
    case "unknownPlaceholder":
      return { message: text.unknownPlaceholder(error.name) }
    case "forbiddenCharacter":
      return { message: text.forbiddenCharacter(error.character) }
  }
}

/** What may fix a failed preview or download: updating the tool that couldn't read a site's page, reading browser
 * cookies when a signed-in account is needed, signing in to the site in a browser and trying again, setting up
 * gallery-dl for a post of images, or adding the password of an archive the download brought. */
export type FailureFix = "updateYtDlp" | "updateGalleryDl" | "cookies" | "signIn" | "setUpImages" | "archivePassword"

export function fixFor(error: unknown): FailureFix | null {
  if (isCoreError<DownloadError>(error) && error.kind === "unpack") {
    const { kind } = error.problem
    return kind === "needsPassword" || kind === "wrongPassword" ? "archivePassword" : null
  }
  const failure = isCoreError<DownloadError>(error) && error.kind === "ytDlp" ? error.error : error
  if (!isCoreError<PreviewError>(failure)) return null
  switch (failure.kind) {
    case "siteError":
      return failure.tool === "galleryDl" ? "updateGalleryDl" : "updateYtDlp"
    case "signInRequired":
      return failure.cookies === "notSignedIn" || failure.cookies === "signedIn" ? "signIn" : "cookies"
    case "imagesNotSetUp":
      return "setUpImages"
    default:
      return null
  }
}

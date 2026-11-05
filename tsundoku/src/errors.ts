import type { DownloadError } from "./bindings/DownloadError"
import type { HistoryError } from "./bindings/HistoryError"
import type { PresetError } from "./bindings/PresetError"
import type { PreviewError } from "./bindings/PreviewError"
import type { ToolError } from "./bindings/ToolError"
import type { ToolId } from "./bindings/ToolId"
import { strings } from "./strings"

/*
  The core's errors are tagged unions (ToolError, PreviewError, HistoryError, PresetError, DownloadError): an action that fails rejects with one.
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

export function describePreviewError(error: unknown): ErrorDescription {
  if (!isCoreError<PreviewError>(error)) return unexpected(error)
  const text = strings.preview.errors
  switch (error.kind) {
    case "invalidLink":
      return { message: text.invalidLink }
    case "playlist":
      return { message: text.playlist }
    case "liveStream":
      return { message: text.liveStream }
    case "upcoming":
      return { message: text.upcoming, detail: error.detail || undefined }
    case "unsupportedSite":
      return { message: text.unsupportedSite }
    case "unavailable":
      return { message: text.unavailable, detail: error.detail }
    case "signInRequired":
      return { message: text.signInRequired, detail: error.detail }
    case "network":
      return { message: text.network, detail: error.detail }
    case "siteError":
      return { message: text.siteError, detail: error.detail }
    case "failed":
      return { message: text.failed, detail: error.detail }
    case "tool":
      return describeToolError(error.error, "ytDlp")
    case "cancelled":
      return { message: strings.errors.cancelled }
  }
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
    case "builtIn":
      return { message: text.builtIn }
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
    case "wrongState":
      return { message: text.wrongState }
    case "storage":
      return { message: text.storage, detail: error.detail }
    case "invalidRateLimit":
      return { message: text.invalidRateLimit, detail: error.value }
    case "unexpected":
      return { message: strings.errors.unexpected, detail: error.detail }
  }
}

/** What may fix a failed preview or download: updating yt-dlp when a site's page couldn't be read, or reading
 * browser cookies when the video needs a signed-in account. */
export type FailureFix = "updateYtDlp" | "cookies"

export function fixFor(error: unknown): FailureFix | null {
  const failure = isCoreError<DownloadError>(error) && error.kind === "ytDlp" ? error.error : error
  if (!isCoreError<PreviewError>(failure)) return null
  switch (failure.kind) {
    case "siteError":
      return "updateYtDlp"
    case "signInRequired":
      return "cookies"
    default:
      return null
  }
}

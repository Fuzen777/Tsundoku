import type { HistoryError } from "./bindings/HistoryError"
import type { PreviewError } from "./bindings/PreviewError"
import type { ToolError } from "./bindings/ToolError"
import type { ToolId } from "./bindings/ToolId"
import { strings } from "./strings"

/*
  The core's errors are tagged unions (ToolError, PreviewError, HistoryError): an action that fails rejects with one.
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

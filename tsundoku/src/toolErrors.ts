import type { ToolError } from "./bindings/ToolError"
import type { ToolId } from "./bindings/ToolId"
import { strings } from "./strings"

export interface ErrorDescription {
  message: string
  /** The technical cause, shown smaller under the message. */
  detail?: string
}

/** Whether an action's error came from the core (a ToolError) rather than from somewhere unexpected. */
export function isToolError(error: unknown): error is ToolError {
  return typeof error === "object" && error !== null && "kind" in error
}

/** Cancelling is the user's choice, so it gets no error message. */
export function isCancelled(error: unknown): boolean {
  return isToolError(error) && error.kind === "cancelled"
}

/** The sentence for a tool's error. */
export function describeToolError(error: unknown, id: ToolId): ErrorDescription {
  const name = strings.tools.names[id]
  const text = strings.tools.errors
  if (!isToolError(error)) return { message: text.unexpected, detail: String(error) }
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
      return { message: text.cancelled }
    case "notManaged":
      return { message: text.notManaged(name) }
    case "storage":
      return { message: text.storage, detail: error.detail }
    case "unexpected":
      return { message: text.unexpected, detail: error.detail }
  }
}

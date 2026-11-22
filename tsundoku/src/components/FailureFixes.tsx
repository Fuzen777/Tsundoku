import { useState } from "react"
import { installTool, setYtDlpChannel, updateGalleryDl, updateYtDlp } from "../api"
import type { ToolId } from "../bindings/ToolId"
import { describeToolError, fixFor, isCancelled, type ErrorDescription } from "../errors"
import { useAppActions } from "../navigation"
import { useDownloadOptions, useStoreToolStatus, useStoreYtDlpChannel, useToolsOverview } from "../queries"
import { strings } from "../strings"
import { Button } from "./Button"
import { PasswordForm } from "./PasswordForm"
import { StatusMessage } from "./StatusMessage"

/**
 * Buttons that may fix a failed preview or download (see `fixFor`): Update yt-dlp, or Try Nightly, or Update
 * gallery-dl, when a site's page couldn't be read; Set Up Cookies when a signed-in account is needed and cookies are
 * off; Try Again once signed in, when no browser is signed in to the site or its sign-in was refused; Sign In to Pixiv,
 * which opens Settings → Sign-Ins, when Pixiv needs Tsundoku's own sign-in; Set Up Image
 * Downloads for a post of images while gallery-dl isn't set up; a field for the password of an archive a download
 * brought, which `onArchivePassword` takes (the queue's row: it retries, which only unpacks). Nothing for other
 * failures.
 */
export function FailureFixes({
  error,
  onRetry,
  onArchivePassword,
}: {
  error: unknown
  onRetry?: () => void
  onArchivePassword?: (password: string) => void
}) {
  const fix = fixFor(error)
  const { navigate } = useAppActions()
  if (fix === "cookies") {
    return (
      <Button size="sm" onClick={() => navigate("settings")}>
        {strings.queue.fixes.setUpCookies}
      </Button>
    )
  }
  if (fix === "signIn") {
    return (
      <div className="flex flex-wrap gap-2">
        {onRetry && (
          <Button size="sm" onClick={onRetry}>
            {strings.queue.fixes.tryAgain}
          </Button>
        )}
        <Button size="sm" variant="ghost" onClick={() => navigate("settings")}>
          {strings.queue.fixes.cookieSettings}
        </Button>
      </div>
    )
  }
  if (fix === "pixivSignIn" || fix === "fanboxSignIn") {
    return (
      <Button size="sm" onClick={() => navigate("settings", "settings-sign-ins")}>
        {fix === "pixivSignIn" ? strings.queue.fixes.signInToPixiv : strings.queue.fixes.signInToFanbox}
      </Button>
    )
  }
  if (fix === "updateYtDlp") return <UpdateYtDlp />
  if (fix === "updateGalleryDl") {
    return <InstallTool tool="galleryDl" label={strings.queue.fixes.updateGalleryDl} updateOwnCopy onDone={onRetry} />
  }
  if (fix === "setUpImages") {
    return <InstallTool tool="galleryDl" label={strings.queue.fixes.setUpImages} onDone={onRetry} />
  }
  if (fix === "archivePassword" && onArchivePassword) return <EnterArchivePassword onEnter={onArchivePassword} />
  return null
}

/** The password of an archive none of the known ones opened: the field and Unlock as a link's (`PasswordForm`), then
 * the download retries with it, which only unpacks. It's never added to Settings' list; the line under the field says
 * whether it's kept as a past password (Save Past Passwords). */
function EnterArchivePassword({ onEnter }: { onEnter: (password: string) => void }) {
  const { navigate } = useAppActions()
  const { data: options } = useDownloadOptions()

  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex flex-wrap items-center gap-2">
        <PasswordForm size="sm" autoFocus={false} onUnlock={onEnter} />
        <Button size="sm" variant="ghost" onClick={() => navigate("settings", "settings-passwords")}>
          {strings.queue.fixes.passwordSettings}
        </Button>
      </div>
      {options && (
        <p className="text-xs text-fg-muted">
          {options.savePastPasswords
            ? strings.queue.fixes.archivePasswordKept
            : strings.queue.fixes.archivePasswordNotKept}
        </p>
      )}
    </div>
  )
}

/**
 * Downloads Tsundoku's own copy of a tool, the latest one, then tries again when the caller can: gallery-dl for a
 * post of images, Deno from the preview card's warning. With `updateOwnCopy` (gallery-dl only), a copy that's already
 * Tsundoku's is updated instead, downloaded only when a newer one exists.
 */
export function InstallTool({
  tool,
  label,
  updateOwnCopy = false,
  onDone,
}: {
  tool: ToolId
  label: string
  updateOwnCopy?: boolean
  onDone?: () => void
}) {
  const { data: tools } = useToolsOverview()
  const storeToolStatus = useStoreToolStatus()
  const [installing, setInstalling] = useState(false)
  const [outcome, setOutcome] = useState<(ErrorDescription & { tone: "success" | "danger" }) | null>(null)
  const isOwnCopy = tools?.tools.find((each) => each.id === tool)?.setup === "managed"

  async function install() {
    setInstalling(true)
    setOutcome(null)
    try {
      const installed =
        updateOwnCopy && isOwnCopy && tool === "galleryDl" ? await updateGalleryDl() : await installTool(tool)
      storeToolStatus(installed)
      if (onDone) onDone()
      else setOutcome({ tone: "success", message: strings.queue.fixes.installed })
    } catch (error) {
      if (!isCancelled(error)) setOutcome({ tone: "danger", ...describeToolError(error, tool) })
    } finally {
      setInstalling(false)
    }
  }

  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-wrap gap-2">
        <Button size="sm" loading={installing} onClick={() => void install()}>
          {label}
        </Button>
      </div>
      {outcome && <StatusMessage {...outcome} />}
    </div>
  )
}

function UpdateYtDlp() {
  const { data: tools } = useToolsOverview()
  const storeToolStatus = useStoreToolStatus()
  const storeYtDlpChannel = useStoreYtDlpChannel()
  const [updating, setUpdating] = useState(false)
  const [outcome, setOutcome] = useState<(ErrorDescription & { tone: "success" | "danger" }) | null>(null)
  const ytDlp = tools?.tools.find((tool) => tool.id === "ytDlp")
  const canTryNightly = ytDlp?.setup === "managed" && tools?.ytDlpChannel === "stable"

  async function update(toNightly: boolean) {
    setUpdating(true)
    setOutcome(null)
    try {
      if (toNightly) {
        await setYtDlpChannel("nightly")
        storeYtDlpChannel("nightly")
      }
      const updated = await updateYtDlp()
      storeToolStatus(updated)
      setOutcome({ tone: "success", message: strings.queue.fixes.updated(updated.version) })
    } catch (error) {
      if (!isCancelled(error)) setOutcome({ tone: "danger", ...describeToolError(error, "ytDlp") })
    } finally {
      setUpdating(false)
    }
  }

  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-wrap gap-2">
        <Button size="sm" loading={updating} onClick={() => void update(false)}>
          {strings.queue.fixes.updateYtDlp}
        </Button>
        {canTryNightly && (
          <Button size="sm" variant="ghost" disabled={updating} onClick={() => void update(true)}>
            {strings.queue.fixes.tryNightly}
          </Button>
        )}
      </div>
      {outcome && <StatusMessage {...outcome} />}
    </div>
  )
}

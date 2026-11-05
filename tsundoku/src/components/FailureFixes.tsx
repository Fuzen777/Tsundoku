import { useState } from "react"
import { installTool, setYtDlpChannel, updateYtDlp } from "../api"
import { describeToolError, fixFor, isCancelled, type ErrorDescription } from "../errors"
import { useAppActions } from "../navigation"
import { useStoreToolStatus, useStoreYtDlpChannel, useToolsOverview } from "../queries"
import { strings } from "../strings"
import { Button } from "./Button"
import { StatusMessage } from "./StatusMessage"

/**
 * Buttons that may fix a failed preview or download (see `fixFor`): Update yt-dlp, or Try Nightly, or Update
 * gallery-dl, when a site's page couldn't be read; Set Up Cookies when a signed-in account is needed and cookies are
 * off; Try Again once signed in, when no browser is signed in to the site or its sign-in was refused; Set Up Image
 * Downloads for a post of images while gallery-dl isn't set up. Nothing for other failures.
 */
export function FailureFixes({ error, onRetry }: { error: unknown; onRetry?: () => void }) {
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
  if (fix === "updateYtDlp") return <UpdateYtDlp />
  if (fix === "updateGalleryDl") return <InstallGalleryDl label={strings.queue.fixes.updateGalleryDl} onDone={onRetry} />
  if (fix === "setUpImages") return <InstallGalleryDl label={strings.queue.fixes.setUpImages} onDone={onRetry} />
  return null
}

/** Downloads Tsundoku's own copy of gallery-dl, the latest one, then tries again when the caller can. */
function InstallGalleryDl({ label, onDone }: { label: string; onDone?: () => void }) {
  const storeToolStatus = useStoreToolStatus()
  const [installing, setInstalling] = useState(false)
  const [outcome, setOutcome] = useState<(ErrorDescription & { tone: "success" | "danger" }) | null>(null)

  async function install() {
    setInstalling(true)
    setOutcome(null)
    try {
      const installed = await installTool("galleryDl")
      storeToolStatus(installed)
      if (onDone) onDone()
      else setOutcome({ tone: "success", message: strings.queue.fixes.installed })
    } catch (error) {
      if (!isCancelled(error)) setOutcome({ tone: "danger", ...describeToolError(error, "galleryDl") })
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

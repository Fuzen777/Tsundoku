import { useState } from "react"
import { setYtDlpChannel, updateYtDlp } from "../api"
import { describeToolError, fixFor, isCancelled, type ErrorDescription } from "../errors"
import { useAppActions } from "../navigation"
import { useStoreToolStatus, useStoreYtDlpChannel, useToolsOverview } from "../queries"
import { strings } from "../strings"
import { Button } from "./Button"
import { StatusMessage } from "./StatusMessage"

/**
 * Buttons that may fix a failed preview or download (see `fixFor`): Update yt-dlp, or Try Nightly, when a site's
 * page couldn't be read; Set Up Cookies when the video needs a signed-in account. Nothing for other failures.
 */
export function FailureFixes({ error }: { error: unknown }) {
  const fix = fixFor(error)
  const { navigate } = useAppActions()
  if (fix === "cookies") {
    return (
      <Button size="sm" onClick={() => navigate("settings")}>
        {strings.queue.fixes.setUpCookies}
      </Button>
    )
  }
  if (fix === "updateYtDlp") return <UpdateYtDlp />
  return null
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

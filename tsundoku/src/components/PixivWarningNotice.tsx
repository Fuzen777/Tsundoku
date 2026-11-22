import { useState } from "react"
import { useQueryClient } from "@tanstack/react-query"
import { ExternalLink, Gauge, TriangleAlert } from "lucide-react"
import { dismissPixivWarning, openWebPage, PIXIV_MESSAGES_PAGE, queryKeys } from "../api"
import { formatTimeAgo } from "../format"
import { useAppActions } from "../navigation"
import { usePixivWarning } from "../queries"
import { strings } from "../strings"
import { Button } from "./Button"
import { Card } from "./Card"
import { LinkButton } from "./LinkButton"
import { StatusMessage } from "./StatusMessage"

const text = strings.pixivWarning

/**
 * Above the view while Pixiv's warning stops its downloads (`pixiv_warning` in the core): what happened, Pixiv's
 * messages and the pacing settings to look at, and Go On, which lets Pixiv's downloads and checks continue.
 */
export function PixivWarningNotice() {
  const queryClient = useQueryClient()
  const { navigate } = useAppActions()
  const { data: status } = usePixivWarning()
  const [going, setGoing] = useState(false)
  const [failed, setFailed] = useState<string | null>(null)
  const warning = status?.warning
  if (!warning) return null

  async function goOn() {
    setFailed(null)
    setGoing(true)
    try {
      queryClient.setQueryData(queryKeys.pixivWarning, await dismissPixivWarning())
    } catch (caught) {
      setFailed(String(caught))
    } finally {
      setGoing(false)
    }
  }

  return (
    <Card className="flex flex-col gap-3 p-4">
      <div className="flex items-center gap-3">
        <TriangleAlert size={18} className="shrink-0 text-danger" />
        <p className="min-w-0 flex-1 font-medium">{text.title}</p>
        <Button size="sm" variant="secondary" loading={going} onClick={() => void goOn()}>
          {text.goOn}
        </Button>
      </div>
      <p className="ml-7.5 text-fg-secondary">{text.body(formatTimeAgo(warning.sentAt / 1000))}</p>
      <div className="ml-7.5 flex flex-wrap gap-x-4 gap-y-1">
        <LinkButton icon={ExternalLink} label={text.openMessages} onClick={() => void openWebPage(PIXIV_MESSAGES_PAGE)} />
        <LinkButton icon={Gauge} label={text.pacing} onClick={() => navigate("settings", "settings-pacing")} />
      </div>
      {failed && <StatusMessage tone="danger" message={text.goOnFailed} detail={failed} />}
    </Card>
  )
}

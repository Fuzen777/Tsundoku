import { Sparkles, X } from "lucide-react"
import { useAppInfo, useDismissWhatsNew, useWhatsNew } from "../queries"
import { strings } from "../strings"
import { Card } from "./Card"
import { IconButton } from "./IconButton"
import { ReleaseNotes } from "./ReleaseNotes"
import { StatusMessage } from "./StatusMessage"

const text = strings.whatsNew

/**
 * Under the add bar after an update: what changed since the version that ran last, from the changelog this version
 * was built with, until it's closed. A first install shows nothing.
 */
export function WhatsNewCard() {
  const { data: sections = [] } = useWhatsNew()
  const { data: appInfo } = useAppInfo()
  const { dismiss, error } = useDismissWhatsNew()
  if (sections.length === 0) return null
  const several = sections.length > 1

  return (
    <Card className="flex flex-col gap-3 p-4">
      <div className="flex items-center gap-3">
        <Sparkles size={18} className="shrink-0 text-accent" />
        <p className="min-w-0 flex-1 font-medium">{text.title(appInfo?.version ?? sections[0].version)}</p>
        <IconButton label={text.close} icon={X} onClick={() => void dismiss()} />
      </div>
      <div className="ml-7.5 flex max-h-72 flex-col gap-4 overflow-y-auto">
        {sections.map((section) => (
          <div key={section.version} className="flex flex-col gap-2">
            {several && <p className="font-semibold">{strings.version(section.version)}</p>}
            <ReleaseNotes markdown={section.body} />
          </div>
        ))}
      </div>
      {error && <StatusMessage tone="danger" {...error} />}
    </Card>
  )
}

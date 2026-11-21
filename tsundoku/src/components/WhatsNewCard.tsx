import { useEffect, useRef, useState, type ReactNode } from "react"
import { Sparkles, X } from "lucide-react"
import type { ErrorDescription } from "../errors"
import { sectionDate, sectionToShow } from "../notes"
import { useAppInfo, useDismissWhatsNew, useReleaseNotes, useWhatsNew } from "../queries"
import { strings } from "../strings"
import { Card } from "./Card"
import { SelectInput } from "./Field"
import { IconButton } from "./IconButton"
import { ReleaseNotes } from "./ReleaseNotes"
import { StatusMessage } from "./StatusMessage"

const text = strings.whatsNew

interface WhatsNewCardProps {
  /** Opened by hand, from Settings → About's What's New: any version's changes, one version at a time. */
  browsing: boolean
  onStopBrowsing: () => void
}

/**
 * Under the add bar after an update: what changed since the version that ran last, from the changelog this version
 * was built with, until it's closed. A first install shows nothing. Opened by hand, it shows the running version's
 * changes instead, with a list of every version to pick another from.
 */
export function WhatsNewCard({ browsing, onStopBrowsing }: WhatsNewCardProps) {
  const { data: sinceLastRun = [] } = useWhatsNew()
  const { data: appInfo } = useAppInfo()
  const { dismiss, error } = useDismissWhatsNew()

  if (browsing) {
    const close = () => {
      onStopBrowsing()
      // It took the place of the card an update left, so closing it closes that one too.
      if (sinceLastRun.length > 0) void dismiss()
    }
    return <BrowsedNotes running={appInfo?.version} onClose={close} error={error} />
  }
  if (sinceLastRun.length === 0) return null
  const several = sinceLastRun.length > 1

  return (
    <NotesCard
      heading={
        <p className="min-w-0 flex-1 font-medium">{text.title(appInfo?.version ?? sinceLastRun[0].version)}</p>
      }
      onClose={() => void dismiss()}
      error={error}
    >
      <div className="ml-7.5 flex max-h-72 flex-col gap-4 overflow-y-auto">
        {sinceLastRun.map((section) => (
          <div key={section.version} className="flex flex-col gap-2">
            {several && <p className="font-semibold">{strings.version(section.version)}</p>}
            <ReleaseNotes markdown={section.body} />
          </div>
        ))}
      </div>
    </NotesCard>
  )
}

interface BrowsedNotesProps {
  /** The running version, whose changes show first. */
  running: string | undefined
  onClose: () => void
  error: ErrorDescription | null
}

/** One version's changes, with the list of versions in the heading: the running one to start with. */
function BrowsedNotes({ running, onClose, error }: BrowsedNotesProps) {
  const { data: sections } = useReleaseNotes()
  const [picked, setPicked] = useState<string | null>(null)
  const versionList = useRef<HTMLSelectElement>(null)
  const loaded = sections !== undefined
  // The card opens far from the button that asked for it, so the keyboard goes to its list: the arrow keys then go
  // through the versions.
  useEffect(() => {
    if (loaded) versionList.current?.focus({ preventScroll: true })
  }, [loaded])
  if (!sections) return null
  const shown = sectionToShow(sections, picked, running)

  return (
    <NotesCard
      heading={
        <div className="flex min-w-0 flex-1 items-center gap-2">
          <p className="truncate font-medium">{text.browseTitle}</p>
          {shown && (
            <SelectInput
              ref={versionList}
              className="shrink-0"
              aria-label={text.version}
              value={shown.version}
              onChange={(event) => setPicked(event.target.value)}
            >
              {sections.map((section) => (
                <option key={section.version} value={section.version}>
                  {text.versionOption(section.version, sectionDate(section))}
                </option>
              ))}
            </SelectInput>
          )}
        </div>
      }
      onClose={onClose}
      error={error}
    >
      {shown ? (
        // Keyed by the version, so another version's changes start at their top.
        <div key={shown.version} className="ml-7.5 max-h-72 overflow-y-auto">
          <ReleaseNotes markdown={shown.body} />
        </div>
      ) : (
        <p className="ml-7.5 text-fg-secondary">{text.none}</p>
      )}
    </NotesCard>
  )
}

interface NotesCardProps {
  /** What the card is about, between its icon and Close. */
  heading: ReactNode
  onClose: () => void
  error: ErrorDescription | null
  children: ReactNode
}

function NotesCard({ heading, onClose, error, children }: NotesCardProps) {
  return (
    <Card className="flex flex-col gap-3 p-4">
      <div className="flex items-center gap-3">
        <Sparkles size={18} className="shrink-0 text-accent" />
        {heading}
        <IconButton label={text.close} icon={X} onClick={onClose} />
      </div>
      {children}
      {error && <StatusMessage tone="danger" {...error} />}
    </Card>
  )
}

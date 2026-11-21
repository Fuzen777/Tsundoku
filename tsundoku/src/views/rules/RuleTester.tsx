import { useRef, useState, type FormEvent, type ReactNode } from "react"
import { Loader2 } from "lucide-react"
import { fetchPreview } from "../../api"
import type { LinkPreview } from "../../bindings/LinkPreview"
import type { VideoInfo } from "../../bindings/VideoInfo"
import { Button } from "../../components/Button"
import { Card } from "../../components/Card"
import { DestinationLine } from "../../components/DestinationLine"
import { TextInput } from "../../components/Field"
import { StatusMessage } from "../../components/StatusMessage"
import { describePreviewError, type ErrorDescription } from "../../errors"
import { joinDetails } from "../../format"
import { useAppActions } from "../../navigation"
import {
  useDownloadDestination,
  usePresets,
  useRuleLabel,
  useRulePreset,
  useRuleRightAway,
  useRuleUnpacking,
} from "../../queries"
import { strings } from "../../strings"
import { scrollWithin } from "../../scrollWithin"

const text = strings.rules.tester

type Trial =
  | { stage: "idle" }
  | { stage: "reading" }
  | { stage: "failed"; error: ErrorDescription }
  | { stage: "ready"; preview: LinkPreview }

/**
 * Test a Link, at the top of Rules: reads a link as the preview card does, then shows where its download would go,
 * the preset it would start as, whether copied and sent links of it start once read, and how its archives would be
 * unpacked, each with the rule that decides it. Apply a Rule…, beside the folder, tries a rule picked by hand, a manual
 * one too, and stays picked for the next link tested. Nothing is downloaded, and the answers follow the rules as
 * they're edited.
 */
export function RuleTester() {
  const [link, setLink] = useState("")
  const [trial, setTrial] = useState<Trial>({ stage: "idle" })
  // A rule applied by hand to the link tested, as Pending and the preview card can.
  const [ruleId, setRuleId] = useState<string | null>(null)
  // A later test's answer wins over an earlier one still on its way.
  const latestTest = useRef(0)

  async function test(event: FormEvent) {
    event.preventDefault()
    if (!link.trim()) return
    const number = ++latestTest.current
    setTrial({ stage: "reading" })
    try {
      const preview = await fetchPreview(link.trim())
      if (number === latestTest.current) setTrial({ stage: "ready", preview })
    } catch (error) {
      if (number === latestTest.current) setTrial({ stage: "failed", error: describePreviewError(error) })
    }
  }

  return (
    <Card className="flex flex-col gap-3 p-4">
      <form className="flex gap-2" onSubmit={(event) => void test(event)}>
        <TextInput
          className="flex-1"
          aria-label={text.title}
          placeholder={text.placeholder}
          value={link}
          onChange={(event) => setLink(event.target.value)}
        />
        <Button type="submit" disabled={!link.trim()} loading={trial.stage === "reading"}>
          {text.test}
        </Button>
      </form>
      {trial.stage === "reading" && (
        <p className="flex items-center gap-1.5 text-fg-secondary">
          <Loader2 size={14} className="shrink-0 animate-spin" />
          {text.reading}
        </p>
      )}
      {trial.stage === "failed" && <StatusMessage tone="danger" {...trial.error} />}
      {trial.stage === "ready" &&
        (trial.preview.kind === "listing" ? (
          <StatusMessage tone="neutral" message={text.listing} />
        ) : (
          <TrialOutcome video={trial.preview.video} ruleId={ruleId} onChooseRule={setRuleId} />
        ))}
    </Card>
  )
}

interface TrialOutcomeProps {
  video: VideoInfo
  /** The rule applied to it by hand, if any. */
  ruleId: string | null
  onChooseRule: (ruleId: string | null) => void
}

/** What the rules decide for a read video: its folder, its preset, Download Right Away and unpacking, each with its
 * rule, the one applied by hand first (Download Right Away is decided before one can be). */
function TrialOutcome({ video, ruleId, onChooseRule }: TrialOutcomeProps) {
  const { navigate } = useAppActions()
  const { data: presets } = usePresets()
  const hasPreset = !video.post && !video.files
  const { data: rulePreset, isPending: pickingPreset } = useRulePreset(video, ruleId)
  const presetId = hasPreset ? (rulePreset?.presetId ?? presets?.defaultId ?? null) : null
  const ready = !hasPreset || (presetId !== null && !pickingPreset)
  const { data: destination } = useDownloadDestination(video, presetId, ready, ruleId)
  const { data: rightAway } = useRuleRightAway(video, presetId, ready)
  const { data: unpacking } = useRuleUnpacking(video, presetId, ready, ruleId)
  const ruleLabel = useRuleLabel()
  const presetName = presets?.presets.find((preset) => preset.id === presetId)?.name
  const ruleLink = (rule: number) => (
    <button
      type="button"
      className="rounded-sm text-fg-muted transition-colors hover:text-fg focus-visible:outline-2 focus-visible:outline-accent"
      onClick={() => {
        const row = document.getElementById(`rule-${rule}`)
        if (row) scrollWithin(row, "center", "smooth")
      }}
    >
      {strings.preview.destinationFrom(text.byRule(ruleLabel(rule)))}
    </button>
  )

  return (
    <div className="flex flex-col gap-2 border-t border-line pt-3">
      <div className="min-w-0">
        <p className="truncate font-medium" title={video.title}>
          {video.title}
        </p>
        <p className="truncate text-xs text-fg-secondary">{joinDetails([video.uploader, video.siteName])}</p>
      </div>
      <dl className="grid grid-cols-[auto_minmax(0,1fr)] items-center gap-x-4 gap-y-1.5 text-xs">
        <Row label={text.savesIn}>
          <DestinationLine
            destination={destination}
            chosenRuleId={ruleId}
            onChooseRule={onChooseRule}
            className="min-h-4"
          />
        </Row>
        <Row label={text.startsAs}>
          {hasPreset ? (
            <span className="flex min-w-0 items-center gap-1.5 text-fg-secondary">
              <span className="truncate">{presetName ?? ""}</span>
              {rulePreset ? (
                ruleLink(rulePreset.rule)
              ) : (
                <button
                  type="button"
                  className="shrink-0 rounded-sm text-fg-muted transition-colors hover:text-fg focus-visible:outline-2 focus-visible:outline-accent"
                  onClick={() => navigate("presets")}
                >
                  {strings.preview.destinationFrom(text.defaultPreset)}
                </button>
              )}
            </span>
          ) : (
            <span className="text-fg-secondary">{text.noPreset}</span>
          )}
        </Row>
        <Row label={text.copiedLinks}>
          {rightAway ? (
            <span className="flex items-center gap-1.5 text-fg-secondary">
              {rightAway.downloadRightAway ? text.rightAway : text.waitInPending}
              {ruleLink(rightAway.rule)}
            </span>
          ) : (
            <span className="text-fg-secondary">{text.asSettingsSay}</span>
          )}
        </Row>
        <Row label={text.archives}>
          {unpacking ? (
            <span className="flex items-center gap-1.5 text-fg-secondary">
              {unpacking.unpacking === "off"
                ? text.leftPacked
                : text.unpackedTo(strings.settings.unpackPlaces[unpacking.unpacking])}
              {ruleLink(unpacking.rule)}
            </span>
          ) : (
            <span className="text-fg-secondary">{text.asArchiveSettingsSay}</span>
          )}
        </Row>
      </dl>
    </div>
  )
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <>
      <dt className="font-medium text-fg-secondary">{label}</dt>
      <dd className="min-w-0">{children}</dd>
    </>
  )
}

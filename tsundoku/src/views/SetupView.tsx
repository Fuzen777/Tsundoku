import { useState } from "react"
import { cancelToolOperation, chooseToolCopy, installTool } from "../api"
import type { FoundCopy } from "../bindings/FoundCopy"
import type { ToolId } from "../bindings/ToolId"
import type { ToolStatus } from "../bindings/ToolStatus"
import { AppTitle } from "../components/AppTitle"
import { Button } from "../components/Button"
import { Card } from "../components/Card"
import { StatusMessage } from "../components/StatusMessage"
import { ToolHeading } from "../components/ToolHeading"
import { ToolStageProgress } from "../components/ToolStageProgress"
import { formatBytes } from "../format"
import { useDetectedTools, useFinishSetup, useToolDownloads } from "../queries"
import { strings } from "../strings"
import { describeToolError, isCancelled } from "../toolErrors"
import { toolOrder } from "../toolInfo"
import { useToolStage } from "../toolProgress"

/** For a tool found on this PC: use that copy, or download Tsundoku's own anyway. */
type Choice = "existing" | "download"

type Outcome = { state: "ready"; status: ToolStatus } | { state: "failed"; error: unknown }

/**
 * The first-run screen, until yt-dlp and ffmpeg are set up. Each tool found on this PC can be used as it is or
 * downloaded fresh; missing ones are downloaded. Deno can be left out if its download fails.
 */
export function SetupView() {
  const detected = useDetectedTools()
  const downloads = useToolDownloads()
  const finishSetup = useFinishSetup()
  const [choices, setChoices] = useState<Partial<Record<ToolId, Choice>>>({})
  const [outcomes, setOutcomes] = useState<Partial<Record<ToolId, Outcome>>>({})
  const [working, setWorking] = useState(false)

  const foundCopy = (id: ToolId) => detected.data?.find((tool) => tool.id === id)?.found ?? null
  const choiceFor = (id: ToolId): Choice => choices[id] ?? (foundCopy(id) ? "existing" : "download")
  const downloadSize = (id: ToolId) => {
    const size = downloads.data?.find((download) => download.id === id)?.size
    return size === undefined || size === null ? null : formatBytes(size)
  }

  const remaining = toolOrder.filter((id) => outcomes[id]?.state !== "ready")
  const hasFailures = remaining.some((id) => outcomes[id]?.state === "failed")
  const onlyDenoFailed = hasFailures && remaining.length === 1 && remaining[0] === "deno"
  const primaryLabel = hasFailures
    ? strings.setup.tryAgain
    : remaining.some((id) => choiceFor(id) === "download")
      ? strings.setup.downloadAndContinue
      : strings.setup.continue

  /** Sets up every tool not ready yet, all at once. Each card shows its outcome as soon as its tool is done. */
  async function setUp() {
    setWorking(true)
    const settled = await Promise.all(
      remaining.map(async (id): Promise<[ToolId, Outcome | undefined]> => {
        const copy = foundCopy(id)
        let outcome: Outcome | undefined
        try {
          const status = await (choiceFor(id) === "existing" && copy ? chooseToolCopy(id, copy.path) : installTool(id))
          outcome = { state: "ready", status }
        } catch (error) {
          // A cancelled tool goes back to its choices, with no error.
          outcome = isCancelled(error) ? undefined : { state: "failed", error }
        }
        setOutcomes((current) => ({ ...current, [id]: outcome }))
        return [id, outcome]
      }),
    )
    setWorking(false)
    const latest = new Map(settled)
    const outcomeOf = (id: ToolId) => (latest.has(id) ? latest.get(id) : outcomes[id])
    if (toolOrder.every((id) => outcomeOf(id)?.state === "ready")) finishSetup()
  }

  function cancel() {
    for (const id of remaining) void cancelToolOperation(id)
  }

  return (
    <div className="flex h-full flex-col">
      <AppTitle />
      <main className="min-h-0 flex-1 overflow-auto border-t border-line bg-surface">
        <div className="mx-auto flex max-w-2xl flex-col gap-4 px-6 py-8">
          <header>
            <h1 className="text-xl font-semibold">{strings.setup.title}</h1>
            <p className="mt-1 text-fg-secondary">{strings.setup.body}</p>
          </header>
          {detected.isPending ? (
            <p className="text-fg-muted">{strings.setup.lookingForTools}</p>
          ) : (
            toolOrder.map((id) => (
              <SetupToolCard
                key={id}
                id={id}
                found={foundCopy(id)}
                size={downloadSize(id)}
                choice={choiceFor(id)}
                onChoose={(choice) => setChoices((current) => ({ ...current, [id]: choice }))}
                outcome={outcomes[id]}
                working={working}
              />
            ))
          )}
          <footer className="flex justify-end gap-2">
            {working ? (
              <Button onClick={cancel}>{strings.setup.cancel}</Button>
            ) : (
              onlyDenoFailed && (
                <Button variant="ghost" onClick={finishSetup}>
                  {strings.setup.continueWithoutDeno}
                </Button>
              )
            )}
            <Button variant="primary" loading={working} disabled={detected.isPending} onClick={() => void setUp()}>
              {primaryLabel}
            </Button>
          </footer>
        </div>
      </main>
    </div>
  )
}

interface SetupToolCardProps {
  id: ToolId
  found: FoundCopy | null
  /** The download's size, formatted, when GitHub could be asked. */
  size: string | null
  choice: Choice
  onChoose: (choice: Choice) => void
  outcome?: Outcome
  working: boolean
}

function SetupToolCard({ id, found, size, choice, onChoose, outcome, working }: SetupToolCardProps) {
  const stage = useToolStage(id)
  return (
    <Card className="flex flex-col gap-3 p-4">
      <ToolHeading id={id} />
      {outcome?.state === "ready" ? (
        <StatusMessage tone="success" message={strings.setup.ready(outcome.status.version)} />
      ) : stage ? (
        <ToolStageProgress stage={stage} />
      ) : found ? (
        // min-w-0: a fieldset is at least as wide as its content by default, so a long path would overflow the card.
        <fieldset disabled={working} className="flex min-w-0 flex-col gap-1">
          <legend className="sr-only">{strings.tools.names[id]}</legend>
          <ChoiceOption
            name={`setup-${id}`}
            checked={choice === "existing"}
            onSelect={() => onChoose("existing")}
            label={strings.setup.useThisCopy}
            detail={strings.setup.foundDetail(found.version, found.path)}
          />
          <ChoiceOption
            name={`setup-${id}`}
            checked={choice === "download"}
            onSelect={() => onChoose("download")}
            label={strings.setup.downloadFresh}
            detail={strings.setup.downloadDetail(size)}
          />
        </fieldset>
      ) : (
        <p className="text-fg-secondary">{strings.setup.notFound(size)}</p>
      )}
      {outcome?.state === "failed" && <StatusMessage tone="danger" {...describeToolError(outcome.error, id)} />}
    </Card>
  )
}

interface ChoiceOptionProps {
  /** Shared by the options of one choice. */
  name: string
  checked: boolean
  onSelect: () => void
  label: string
  detail: string
}

/** A radio option with a label and a line of detail, the whole row clickable. */
function ChoiceOption({ name, checked, onSelect, label, detail }: ChoiceOptionProps) {
  return (
    <label className="flex gap-3 rounded-md px-2 py-1.5 transition-colors hover:bg-input has-disabled:opacity-60 has-disabled:hover:bg-transparent">
      <input type="radio" name={name} checked={checked} onChange={onSelect} className="mt-1 accent-accent" />
      <span className="min-w-0">
        <span className="block font-medium">{label}</span>
        <span className="block truncate text-xs text-fg-secondary" title={detail}>
          {detail}
        </span>
      </span>
    </label>
  )
}

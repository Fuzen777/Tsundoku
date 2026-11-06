import { useState, type ReactNode } from "react"
import { useQueryClient } from "@tanstack/react-query"
import { FolderDown, HardDrive, Wrench, type LucideIcon } from "lucide-react"
import { cancelToolOperation, chooseToolCopy, installTool, pickFolder, queryKeys, saveFirstRunChoices } from "../api"
import type { DownloadFolderChoice } from "../bindings/DownloadFolderChoice"
import type { FirstRunOptions } from "../bindings/FirstRunOptions"
import type { FoundCopy } from "../bindings/FoundCopy"
import type { ToolCaches } from "../bindings/ToolCaches"
import type { ToolId } from "../bindings/ToolId"
import type { ToolStatus } from "../bindings/ToolStatus"
import { AppTitle } from "../components/AppTitle"
import { Button } from "../components/Button"
import { Card } from "../components/Card"
import { StatusMessage } from "../components/StatusMessage"
import { IconHeading, ToolHeading } from "../components/ToolHeading"
import { ToolStageProgress } from "../components/ToolStageProgress"
import { formatBytes } from "../format"
import { useDetectedTools, useFinishSetup, useFirstRunOptions, useToolDownloads } from "../queries"
import { strings } from "../strings"
import { describeDownloadError, describeToolError, isCancelled, type ErrorDescription } from "../errors"
import { setupToolOrder } from "../toolInfo"
import { useToolStage } from "../toolProgress"

/** For a tool found on this PC: use that copy, or download Tsundoku's own anyway. */
type Choice = "existing" | "download"

type Outcome = { state: "ready"; status: ToolStatus } | { state: "failed"; error: unknown }

/**
 * The first-run setup, until yt-dlp and ffmpeg are set up. On a new install it first asks where the tools, their caches
 * and downloads go (`ChoicesScreen`), then sets up the tools (`ToolsScreen`).
 */
export function SetupView() {
  const queryClient = useQueryClient()
  const { data: options } = useFirstRunOptions()
  if (!options) return <SetupPage title={strings.setup.choicesTitle} body={strings.setup.choicesBody} />
  if (options.needed) return <ChoicesScreen options={options} />
  const showChoices = () => queryClient.setQueryData<FirstRunOptions>(queryKeys.firstRunOptions, { ...options, needed: true })
  return <ToolsScreen onBack={showChoices} />
}

interface SetupPageProps {
  title: string
  body: string
  children?: ReactNode
  /** The buttons at the bottom right. */
  footer?: ReactNode
}

/** A screen of the first-run setup: the app name, then a centered column with the title, a sentence, the cards and the
 * buttons. */
function SetupPage({ title, body, children, footer }: SetupPageProps) {
  return (
    <div className="flex h-full flex-col">
      <AppTitle />
      <main className="min-h-0 flex-1 overflow-auto border-t border-line bg-surface">
        <div className="mx-auto flex max-w-2xl flex-col gap-4 px-6 py-8">
          <header>
            <h1 className="text-xl font-semibold">{title}</h1>
            <p className="mt-1 text-fg-secondary">{body}</p>
          </header>
          {children}
          {footer && <footer className="flex justify-end gap-2">{footer}</footer>}
        </div>
      </main>
    </div>
  )
}

/**
 * Where the tools go (Tsundoku's data folder unless another is picked), where they keep their caches, and where
 * downloads go. The last two start unpicked, so nothing lands anywhere the user didn't choose.
 */
function ChoicesScreen({ options }: { options: FirstRunOptions }) {
  const queryClient = useQueryClient()
  const text = strings.setup
  /** A folder picked for the tools, or null for the data folder. */
  const [toolsFolder, setToolsFolder] = useState<string | null>(null)
  const [caches, setCaches] = useState<ToolCaches | null>(null)
  const [downloadFolder, setDownloadFolder] = useState<DownloadFolderChoice | null>(null)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<ErrorDescription | null>(null)
  const chosenDownloadFolder = downloadFolder?.kind === "chosen" ? downloadFolder.path : null

  async function chooseToolsFolder() {
    const chosen = await pickFolder(text.toolsFolderTitle)
    if (chosen) setToolsFolder(chosen)
  }

  async function chooseDownloadFolder() {
    const chosen = await pickFolder(text.downloadFolderTitle)
    if (chosen) setDownloadFolder({ kind: "chosen", path: chosen })
  }

  async function save() {
    if (caches === null || downloadFolder === null) return
    setSaving(true)
    setError(null)
    try {
      await saveFirstRunChoices({ toolsFolder, caches, downloadFolder })
      // The tools are looked for again, in the folder picked for them.
      queryClient.removeQueries({ queryKey: queryKeys.detectedTools })
      queryClient.setQueryData<FirstRunOptions>(queryKeys.firstRunOptions, { ...options, needed: false })
    } catch (caught) {
      setError(describeDownloadError(caught))
      setSaving(false)
    }
  }

  return (
    <SetupPage
      title={text.choicesTitle}
      body={text.choicesBody}
      footer={
        <Button
          variant="primary"
          loading={saving}
          disabled={caches === null || downloadFolder === null}
          onClick={() => void save()}
        >
          {text.continue}
        </Button>
      }
    >
      <ChoiceCard icon={Wrench} title={text.toolsFolder} description={text.toolsFolderHint}>
        <ChoiceOption
          name="setup-tools-folder"
          checked={toolsFolder === null}
          onSelect={() => setToolsFolder(null)}
          label={text.toolsInDataFolder}
          detail={options.toolsFolder}
        />
        <ChoiceOption
          name="setup-tools-folder"
          checked={toolsFolder !== null}
          onSelect={() => void chooseToolsFolder()}
          label={text.chooseFolder}
          detail={toolsFolder ?? text.noFolderChosen}
        />
      </ChoiceCard>
      <ChoiceCard icon={HardDrive} title={text.caches} description={text.cachesHint}>
        <ChoiceOption
          name="setup-caches"
          checked={caches === "dataFolder"}
          onSelect={() => setCaches("dataFolder")}
          label={text.cachesInDataFolder}
          detail={options.cacheFolder}
        />
        <ChoiceOption
          name="setup-caches"
          checked={caches === "usualFolders"}
          onSelect={() => setCaches("usualFolders")}
          label={text.cachesInUsualFolders}
          detail={text.cachesUsualDetail}
        />
      </ChoiceCard>
      <ChoiceCard icon={FolderDown} title={text.downloadFolder} description={text.downloadFolderHint}>
        {options.besideAppFolder !== null && (
          <ChoiceOption
            name="setup-download-folder"
            checked={downloadFolder?.kind === "besideApp"}
            onSelect={() => setDownloadFolder({ kind: "besideApp" })}
            label={text.besideApp}
            detail={text.besideAppDetail(options.besideAppFolder)}
          />
        )}
        <ChoiceOption
          name="setup-download-folder"
          checked={downloadFolder?.kind === "default"}
          onSelect={() => setDownloadFolder({ kind: "default" })}
          label={text.inDownloads}
          detail={options.defaultDownloadFolder}
        />
        <ChoiceOption
          name="setup-download-folder"
          checked={chosenDownloadFolder !== null}
          onSelect={() => void chooseDownloadFolder()}
          label={text.chooseFolder}
          detail={chosenDownloadFolder ?? text.noFolderChosen}
        />
      </ChoiceCard>
      {error && <StatusMessage tone="danger" {...error} />}
    </SetupPage>
  )
}

interface ChoiceCardProps {
  icon: LucideIcon
  title: string
  description: string
  /** Its options (`ChoiceOption`). */
  children: ReactNode
}

function ChoiceCard({ icon, title, description, children }: ChoiceCardProps) {
  return (
    <Card className="flex flex-col gap-3 p-4">
      <IconHeading icon={icon} title={title} description={description} />
      {/* min-w-0: a fieldset is at least as wide as its content by default, so a long path would overflow the card. */}
      <fieldset className="flex min-w-0 flex-col gap-1">
        <legend className="sr-only">{title}</legend>
        {children}
      </fieldset>
    </Card>
  )
}

/**
 * The tools: each one found on this PC can be used as it is or downloaded fresh; missing ones are downloaded. Deno can
 * be left out if its download fails. Back returns to the choices until a tool is set up.
 */
function ToolsScreen({ onBack }: { onBack: () => void }) {
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

  const remaining = setupToolOrder.filter((id) => outcomes[id]?.state !== "ready")
  const hasFailures = remaining.some((id) => outcomes[id]?.state === "failed")
  const onlyDenoFailed = hasFailures && remaining.length === 1 && remaining[0] === "deno"
  const anyReady = remaining.length < setupToolOrder.length
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
    if (setupToolOrder.every((id) => outcomeOf(id)?.state === "ready")) finishSetup()
  }

  function cancel() {
    for (const id of remaining) void cancelToolOperation(id)
  }

  return (
    <SetupPage
      title={strings.setup.title}
      body={strings.setup.body}
      footer={
        <>
          {working ? (
            <Button onClick={cancel}>{strings.setup.cancel}</Button>
          ) : (
            <>
              {!anyReady && (
                <Button variant="ghost" onClick={onBack}>
                  {strings.setup.back}
                </Button>
              )}
              {onlyDenoFailed && (
                <Button variant="ghost" onClick={finishSetup}>
                  {strings.setup.continueWithoutDeno}
                </Button>
              )}
            </>
          )}
          <Button variant="primary" loading={working} disabled={detected.isPending} onClick={() => void setUp()}>
            {primaryLabel}
          </Button>
        </>
      }
    >
      {detected.isPending ? (
        <p className="text-fg-muted">{strings.setup.lookingForTools}</p>
      ) : (
        setupToolOrder.map((id) => (
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
    </SetupPage>
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

/** A radio option with a label and a line of detail, the whole row clickable. A row that opens a folder picker stays
 * unchecked until a folder is picked; clicking it again picks another. */
function ChoiceOption({ name, checked, onSelect, label, detail }: ChoiceOptionProps) {
  return (
    <label className="flex gap-3 rounded-md px-2 py-1.5 transition-colors hover:bg-input has-disabled:opacity-60 has-disabled:hover:bg-transparent">
      <input
        type="radio"
        name={name}
        checked={checked}
        onChange={onSelect}
        onClick={checked ? onSelect : undefined}
        className="mt-1 accent-accent"
      />
      <span className="min-w-0">
        <span className="block font-medium">{label}</span>
        <span className="block truncate text-xs text-fg-secondary" title={detail}>
          {detail}
        </span>
      </span>
    </label>
  )
}

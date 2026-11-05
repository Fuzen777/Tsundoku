import { useState } from "react"
import { useMutation } from "@tanstack/react-query"
import {
  cancelToolOperation,
  checkToolUpdate,
  chooseToolCopy,
  installTool,
  pickProgram,
  setYtDlpChannel,
  updateYtDlp,
} from "../../api"
import type { ToolSetupKind } from "../../bindings/ToolSetupKind"
import type { ToolStatus } from "../../bindings/ToolStatus"
import type { UpdateChannel } from "../../bindings/UpdateChannel"
import { Button } from "../../components/Button"
import { Card } from "../../components/Card"
import { Section } from "../../components/Section"
import { SegmentedControl } from "../../components/SegmentedControl"
import { StatusMessage, type StatusTone } from "../../components/StatusMessage"
import { ToolHeading } from "../../components/ToolHeading"
import { ToolStageProgress } from "../../components/ToolStageProgress"
import { useStoreToolStatus, useStoreYtDlpChannel, useToolsOverview } from "../../queries"
import { strings } from "../../strings"
import { describeToolError, isCancelled } from "../../errors"
import { useToolStage } from "../../toolProgress"

const CHANNEL_OPTIONS = [
  { value: "stable", label: strings.settings.channels.stable },
  { value: "nightly", label: strings.settings.channels.nightly },
] as const satisfies readonly { value: UpdateChannel; label: string }[]

/** Settings → Tools: each tool's copy, version and updates. */
export function ToolsSection() {
  const overview = useToolsOverview()
  return (
    <Section title={strings.settings.tools}>
      {overview.data ? (
        overview.data.tools.map((status) => (
          <ToolCard key={status.id} status={status} channel={overview.data.ytDlpChannel} />
        ))
      ) : overview.isError ? (
        <StatusMessage tone="danger" message={strings.errors.unexpected} detail={String(overview.error)} />
      ) : (
        <p className="text-fg-muted">{strings.settings.loadingTools}</p>
      )}
    </Section>
  )
}

interface Feedback {
  tone: StatusTone
  message: string
  detail?: string
}

function ToolCard({ status, channel }: { status: ToolStatus; channel: UpdateChannel }) {
  const { id } = status
  const name = strings.tools.names[id]
  const stage = useToolStage(id)
  const storeToolStatus = useStoreToolStatus()
  const storeYtDlpChannel = useStoreYtDlpChannel()
  const [feedback, setFeedback] = useState<Feedback | null>(null)
  /** The latest release, once a check found it newer than Tsundoku's copy. */
  const [availableUpdate, setAvailableUpdate] = useState<string | null>(null)

  const showError = (error: unknown) => {
    if (!isCancelled(error)) setFeedback({ tone: "danger", ...describeToolError(error, id) })
  }
  const clearFeedback = () => setFeedback(null)

  const install = useMutation({
    mutationFn: () => installTool(id),
    onMutate: clearFeedback,
    onSuccess: (installed) => {
      storeToolStatus(installed)
      setAvailableUpdate(null)
      setFeedback({ tone: "success", message: strings.settings.installed(installed.version) })
    },
    onError: showError,
  })
  const update = useMutation({
    mutationFn: updateYtDlp,
    onMutate: () => {
      clearFeedback()
      return { previousVersion: status.version }
    },
    onSuccess: (updated, _variables, context) => {
      storeToolStatus(updated)
      const unchanged = updated.version === context.previousVersion
      setFeedback({
        tone: "success",
        message: unchanged ? strings.settings.upToDate : strings.settings.updatedTo(updated.version),
      })
    },
    onError: showError,
  })
  const check = useMutation({
    mutationFn: () => checkToolUpdate(id),
    onMutate: clearFeedback,
    onSuccess: (result) => {
      if (result.updateAvailable) {
        setAvailableUpdate(result.latest)
      } else {
        setFeedback({ tone: "success", message: strings.settings.upToDate })
      }
    },
    onError: showError,
  })
  const choose = useMutation({
    mutationFn: (path: string) => chooseToolCopy(id, path),
    onMutate: clearFeedback,
    onSuccess: (chosen) => {
      storeToolStatus(chosen)
      setAvailableUpdate(null)
    },
    onError: showError,
  })
  const changeChannel = useMutation({
    mutationFn: setYtDlpChannel,
    onSuccess: (_result, newChannel) => {
      storeYtDlpChannel(newChannel)
      // Tsundoku's copy moves to the new channel right away.
      if (status.setup === "managed") update.mutate()
    },
    onError: showError,
  })

  async function chooseFile() {
    const path = await pickProgram(strings.settings.chooseFileTitle(name), strings.settings.programsFilter)
    if (path) choose.mutate(path)
  }

  const busy = stage !== undefined || install.isPending || update.isPending || check.isPending || choose.isPending
  const isManaged = status.setup === "managed"
  const needsDownload = status.setup === null || (isManaged && status.problem !== null)

  function primaryAction() {
    if (needsDownload) {
      return (
        <Button size="sm" variant="primary" disabled={busy} onClick={() => install.mutate()}>
          {strings.settings.download}
        </Button>
      )
    }
    if (!isManaged) {
      return (
        <Button size="sm" disabled={busy} onClick={() => install.mutate()}>
          {strings.settings.useOwnCopy}
        </Button>
      )
    }
    if (id === "ytDlp") {
      return (
        <Button size="sm" disabled={busy} onClick={() => update.mutate()}>
          {strings.settings.updateNow}
        </Button>
      )
    }
    if (availableUpdate) {
      return (
        <Button size="sm" variant="primary" disabled={busy} onClick={() => install.mutate()}>
          {strings.settings.update}
        </Button>
      )
    }
    return (
      <Button size="sm" loading={check.isPending} disabled={busy} onClick={() => check.mutate()}>
        {strings.settings.checkForUpdates}
      </Button>
    )
  }

  return (
    <Card className="flex flex-col gap-3 p-4">
      <ToolHeading id={id}>{status.setup && <SourceBadge setup={status.setup} />}</ToolHeading>
      <ToolDetails status={status} />
      {stage ? (
        <ToolStageProgress stage={stage} />
      ) : (
        <>
          {feedback && <StatusMessage {...feedback} />}
          {availableUpdate && (
            <StatusMessage tone="neutral" message={strings.settings.updateAvailable(availableUpdate)} />
          )}
        </>
      )}
      <div className="flex flex-wrap items-center gap-2">
        {stage ? (
          <Button size="sm" onClick={() => void cancelToolOperation(id)}>
            {strings.settings.cancel}
          </Button>
        ) : (
          <>
            {primaryAction()}
            <Button size="sm" variant="ghost" loading={choose.isPending} disabled={busy} onClick={() => void chooseFile()}>
              {strings.settings.chooseFile}
            </Button>
          </>
        )}
      </div>
      {id === "ytDlp" && isManaged && (
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1 border-t border-line pt-3">
          <span>{strings.settings.channel}</span>
          <SegmentedControl
            label={strings.settings.channel}
            options={CHANNEL_OPTIONS}
            value={channel}
            disabled={busy || changeChannel.isPending}
            onChange={(newChannel) => changeChannel.mutate(newChannel)}
          />
          <span className="text-xs text-fg-muted">{strings.settings.channelHint}</span>
        </div>
      )}
    </Card>
  )
}

/** The version and path of the copy that's set up, or why it doesn't run. */
function ToolDetails({ status }: { status: ToolStatus }) {
  if (status.setup === null) return <p className="text-fg-muted">{strings.tools.notSetUp}</p>
  return (
    <div className="flex flex-col gap-0.5">
      {status.problem ? (
        <StatusMessage tone="danger" {...describeToolError(status.problem, status.id)} />
      ) : (
        status.version && <p>{strings.version(status.version)}</p>
      )}
      {status.path && (
        <p className="truncate text-xs text-fg-muted select-text" title={status.path}>
          {status.path}
        </p>
      )}
    </div>
  )
}

function SourceBadge({ setup }: { setup: ToolSetupKind }) {
  return (
    <span className="shrink-0 rounded-full border border-line px-2 py-0.5 text-xs text-fg-secondary">
      {setup === "managed" ? strings.tools.managed : strings.tools.external}
    </span>
  )
}

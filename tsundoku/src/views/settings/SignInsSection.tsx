import { useEffect, useState } from "react"
import { useQueryClient } from "@tanstack/react-query"
import { checkPixivMessages, onPixivSignInFailed, queryKeys, signInToPixiv, signOutOfPixiv } from "../../api"
import type { AccountError } from "../../bindings/AccountError"
import { Button } from "../../components/Button"
import { Card } from "../../components/Card"
import { Section } from "../../components/Section"
import { StatusMessage } from "../../components/StatusMessage"
import { formatTimeAgo } from "../../format"
import { usePixivAccount, usePixivWarning } from "../../queries"
import { strings } from "../../strings"

const text = strings.settings.signIns

/**
 * Sign-ins Tsundoku keeps itself, for sites a browser's cookies can't open: Pixiv's. Sign In opens Pixiv's login page in
 * a window of Tsundoku's (`accounts.rs`); the app's event updates the line once it's done, and a failure says why
 * under it. While the window is open, the line says to finish there. Signed in, a line under it tells the check of
 * Pixiv's messages for a warning (`pixiv_messages.rs`), with Check Now.
 */
export function SignInsSection() {
  const queryClient = useQueryClient()
  const { data: account } = usePixivAccount()
  const [waiting, setWaiting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    const failed = onPixivSignInFailed((failure) => {
      setWaiting(false)
      setError(describeAccountError(failure))
    })
    return () => void failed.then((unlisten) => unlisten())
  }, [])

  async function signIn() {
    setError(null)
    try {
      await signInToPixiv()
      setWaiting(true)
    } catch (caught) {
      setError(String(caught))
    }
  }

  async function signOut() {
    setError(null)
    setWaiting(false)
    try {
      await signOutOfPixiv()
      queryClient.setQueryData(queryKeys.pixivAccount, null)
    } catch (caught) {
      setError(isAccountError(caught) ? describeAccountError(caught) : String(caught))
    }
  }

  return (
    <Section title={text.title}>
      <Card className="flex flex-col gap-3 p-4">
        <div className="flex items-center gap-3">
          <div className="min-w-0 flex-1">
            <p className="font-medium">{text.pixiv}</p>
            <p className="truncate text-fg-secondary">
              {account ? text.signedInAs(account.name, account.account) : text.signedOut}
            </p>
          </div>
          {account ? (
            <Button size="sm" onClick={() => void signOut()}>
              {text.signOut}
            </Button>
          ) : (
            <Button size="sm" variant="primary" onClick={() => void signIn()}>
              {text.signIn}
            </Button>
          )}
        </div>
        <p className="text-xs text-fg-muted">{text.pixivHint}</p>
        {waiting && !account && <StatusMessage tone="neutral" message={text.waiting} />}
        {error && <StatusMessage tone="danger" message={error} />}
        {account && <WarningCheck onSignIn={() => void signIn()} />}
      </Card>
    </Section>
  )
}

/** Where the check of Pixiv's messages for a warning stands, with Check Now, under a divider. */
function WarningCheck({ onSignIn }: { onSignIn: () => void }) {
  const queryClient = useQueryClient()
  const { data: status } = usePixivWarning()
  const [failed, setFailed] = useState<string | null>(null)
  if (!status) return null

  async function checkNow() {
    setFailed(null)
    try {
      queryClient.setQueryData(queryKeys.pixivWarning, await checkPixivMessages())
    } catch (caught) {
      setFailed(String(caught))
    }
  }

  const line = status.checking
    ? text.checking
    : status.checkedAt === null
      ? text.notChecked
      : text.checked(formatTimeAgo(status.checkedAt / 1000))
  return (
    <div className="flex flex-col gap-2 border-t border-line pt-3">
      <div className="flex items-center gap-3">
        <p className="min-w-0 flex-1 text-fg-secondary">{status.warning ? text.warned : line}</p>
        <Button size="sm" variant="ghost" loading={status.checking} onClick={() => void checkNow()}>
          {text.checkNow}
        </Button>
      </div>
      <p className="text-xs text-fg-muted">{text.checkHint}</p>
      {status.problem?.kind === "notSignedIn" && (
        <StatusMessage tone="warning" message={text.notSignedIn}>
          <Button size="sm" onClick={onSignIn}>
            {text.signInAgain}
          </Button>
        </StatusMessage>
      )}
      {status.problem?.kind === "failed" && (
        <StatusMessage tone="warning" message={text.checkFailed} detail={status.problem.detail} />
      )}
      {failed && <StatusMessage tone="danger" message={text.checkFailed} detail={failed} />}
    </div>
  )
}

function isAccountError(value: unknown): value is AccountError {
  return typeof value === "object" && value !== null && "kind" in value
}

function describeAccountError(error: AccountError): string {
  return text.errors[error.kind]
}

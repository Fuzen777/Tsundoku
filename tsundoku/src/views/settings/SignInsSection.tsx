import { useEffect, useState } from "react"
import { useQueryClient } from "@tanstack/react-query"
import { onPixivSignInFailed, queryKeys, signInToPixiv, signOutOfPixiv } from "../../api"
import type { AccountError } from "../../bindings/AccountError"
import { Button } from "../../components/Button"
import { Card } from "../../components/Card"
import { Section } from "../../components/Section"
import { StatusMessage } from "../../components/StatusMessage"
import { usePixivAccount } from "../../queries"
import { strings } from "../../strings"

const text = strings.settings.signIns

/**
 * Sign-ins Tsundoku keeps itself, for sites a browser's cookies can't open: Pixiv's. Sign In opens Pixiv's login page in
 * a window of Tsundoku's (`accounts.rs`); the app's event updates the line once it's done, and a failure says why
 * under it. While the window is open, the line says to finish there.
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
      </Card>
    </Section>
  )
}

function isAccountError(value: unknown): value is AccountError {
  return typeof value === "object" && value !== null && "kind" in value
}

function describeAccountError(error: AccountError): string {
  return text.errors[error.kind]
}

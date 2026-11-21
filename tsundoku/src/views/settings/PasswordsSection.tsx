import { useRef, useState } from "react"
import { confirmWithUser, forgetPastPasswords } from "../../api"
import { Button } from "../../components/Button"
import { Card } from "../../components/Card"
import { Checkbox, Field } from "../../components/Field"
import { HelpMenu } from "../../components/HelpMenu"
import { Section } from "../../components/Section"
import { StatusMessage } from "../../components/StatusMessage"
import { TagInput } from "../../components/TagInput"
import { describeDownloadError, type ErrorDescription } from "../../errors"
import { usePastPasswordCount, useSaveDownloadOptions } from "../../queries"
import { strings } from "../../strings"

const text = strings.settings

/** Settings → Passwords: the passwords tried by themselves on a link or an archive that needs one, before asking. The
 * person's own list first, then the past ones, kept apart from it while Save Past Passwords is on, with how many are
 * kept and Forget Past Passwords. How they're tried is in its (?), How Passwords Are Tried. */
export function PasswordsSection() {
  const { options, save, error } = useSaveDownloadOptions()
  const { count: pastCount, forgotten } = usePastPasswordCount()
  // Each tag added or removed is saved at once, and shown while it is, so a quick second one builds on the first.
  const [savingPasswords, setSavingPasswords] = useState<string[] | null>(null)
  const latestSave = useRef(0)
  const [saved, setSaved] = useState(false)
  const [forgetError, setForgetError] = useState<ErrorDescription | null>(null)
  const listed = options?.unpack.passwords ?? []
  const passwords = savingPasswords ?? listed

  async function savePasswords(next: string[]) {
    if (!options) return
    const number = ++latestSave.current
    setSavingPasswords(next)
    setSaved(false)
    const worked = await save({ unpack: { ...options.unpack, passwords: next } })
    if (number !== latestSave.current) return
    setSavingPasswords(null)
    setSaved(worked)
  }

  async function forget() {
    setForgetError(null)
    const confirmed = await confirmWithUser({
      title: text.forgetPastPasswords,
      message: text.forgetPastPasswordsQuestion(pastCount),
      confirmLabel: text.forgetPastPasswords,
      cancelLabel: strings.dialogs.cancel,
    })
    if (!confirmed) return
    try {
      await forgetPastPasswords()
      forgotten()
    } catch (caught) {
      setForgetError({ ...describeDownloadError(caught), message: text.forgetPastPasswordsFailed })
    }
  }

  return (
    <Section
      title={text.passwords}
      actions={
        <HelpMenu
          label={text.passwordsHelp.label}
          topics={[
            { title: text.passwordsHelp.order, paragraphs: text.passwordsHelp.orderParagraphs },
            { title: text.passwordsHelp.past, paragraphs: text.passwordsHelp.pastParagraphs },
          ]}
        />
      }
    >
      <Card className="flex flex-col gap-4 p-4">
        <Field label={text.passwordList} hint={text.passwordListHint} htmlFor="settings-password-list">
          <TagInput
            id="settings-password-list"
            values={passwords}
            placeholder={text.passwordListPlaceholder}
            removeLabel={text.removePassword}
            disabled={!options}
            onChange={(next) => void savePasswords(next)}
          />
        </Field>
        {saved && <StatusMessage tone="success" message={text.passwordListSaved(listed.length)} />}
        <div className="flex flex-col gap-1">
          <Checkbox
            label={text.savePastPasswords}
            checked={options?.savePastPasswords ?? false}
            disabled={!options}
            onChange={(savePastPasswords) => void save({ savePastPasswords })}
          />
          <p className="pl-6 text-xs text-fg-muted">{text.savePastPasswordsHint}</p>
          {pastCount > 0 && (
            <div className="mt-1 flex flex-wrap items-center gap-3 pl-6">
              <span className="text-fg-secondary">{text.pastPasswordsKept(pastCount)}</span>
              <Button size="sm" variant="secondary" onClick={() => void forget()}>
                {text.forgetPastPasswords}
              </Button>
            </div>
          )}
        </div>
        {forgetError && <StatusMessage tone="danger" {...forgetError} />}
        {error && <StatusMessage tone="danger" {...error} />}
      </Card>
    </Section>
  )
}

import { History } from "lucide-react"
import { EmptyState } from "../components/EmptyState"
import { Page } from "../components/Page"
import { strings } from "../strings"

export function HistoryView() {
  return (
    <Page title={strings.nav.history}>
      <EmptyState icon={History} title={strings.history.emptyTitle} body={strings.history.emptyBody} />
    </Page>
  )
}

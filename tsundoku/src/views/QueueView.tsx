import { ListVideo } from "lucide-react"
import { EmptyState } from "../components/EmptyState"
import { Page } from "../components/Page"
import { strings } from "../strings"

export function QueueView() {
  return (
    <Page title={strings.nav.queue}>
      <EmptyState icon={ListVideo} title={strings.queue.emptyTitle} body={strings.queue.emptyBody} />
    </Page>
  )
}

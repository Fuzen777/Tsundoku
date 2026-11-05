import { Button } from "../components/Button"
import { Card } from "../components/Card"
import { Page } from "../components/Page"
import { Section } from "../components/Section"
import { StatusMessage } from "../components/StatusMessage"
import { useAppInfo, useAppUpdate } from "../queries"
import { strings } from "../strings"
import { CookiesSection, DownloadsSection } from "./settings/DownloadsSection"
import { ToolsSection } from "./settings/ToolsSection"

export function SettingsView() {
  return (
    <Page title={strings.nav.settings}>
      <div className="flex max-w-3xl flex-col gap-6">
        <DownloadsSection />
        <CookiesSection />
        <ToolsSection />
        <AboutSection />
      </div>
    </Page>
  )
}

/** The version, and Check for Updates. A newer release shows as the banner under the add bar. */
function AboutSection() {
  const { data: appInfo } = useAppInfo()
  const update = useAppUpdate()
  const checked = update.isFetchedAfterMount
  return (
    <Section title={strings.settings.about}>
      <Card className="flex flex-col gap-3 px-4 py-3">
        <div className="flex items-center gap-3">
          <div className="min-w-0 flex-1">
            <p className="font-medium">{strings.appName}</p>
            {appInfo && <p className="text-fg-secondary">{strings.version(appInfo.version)}</p>}
          </div>
          <Button size="sm" loading={update.isFetching} onClick={() => void update.refetch()}>
            {strings.appUpdate.check}
          </Button>
        </div>
        {checked && !update.isFetching && update.isError && (
          <StatusMessage tone="danger" message={strings.appUpdate.checkFailed} detail={String(update.error)} />
        )}
        {checked && !update.isFetching && update.isSuccess && update.data === null && (
          <StatusMessage tone="success" message={strings.appUpdate.upToDate} />
        )}
        {checked && !update.isFetching && update.data && (
          <StatusMessage tone="neutral" message={strings.appUpdate.available(update.data.version)} />
        )}
      </Card>
    </Section>
  )
}

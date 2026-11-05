import { Card } from "../components/Card"
import { Page } from "../components/Page"
import { Section } from "../components/Section"
import { useAppInfo } from "../queries"
import { strings } from "../strings"
import { CookiesSection, DownloadsSection } from "./settings/DownloadsSection"
import { ToolsSection } from "./settings/ToolsSection"

export function SettingsView() {
  const { data: appInfo } = useAppInfo()
  return (
    <Page title={strings.nav.settings}>
      <div className="flex max-w-3xl flex-col gap-6">
        <DownloadsSection />
        <CookiesSection />
        <ToolsSection />
        <Section title={strings.settings.about}>
          <Card className="px-4 py-3">
            <p className="font-medium">{strings.appName}</p>
            {appInfo && <p className="text-fg-secondary">{strings.version(appInfo.version)}</p>}
          </Card>
        </Section>
      </div>
    </Page>
  )
}

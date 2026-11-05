import { Page } from "../components/Page"
import { useAppInfo } from "../queries"
import { strings } from "../strings"

export function SettingsView() {
  const { data: appInfo } = useAppInfo()
  return (
    <Page title={strings.nav.settings}>
      <section>
        <h2 className="mb-2 text-sm font-semibold">{strings.settings.about}</h2>
        <div className="rounded-lg border border-line bg-input px-4 py-3">
          <p className="font-medium">{strings.appName}</p>
          {appInfo && <p className="text-fg-secondary">{strings.settings.version(appInfo.version)}</p>}
        </div>
      </section>
    </Page>
  )
}

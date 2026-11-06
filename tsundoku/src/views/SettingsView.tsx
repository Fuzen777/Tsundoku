import { useState } from "react"
import { openDataFolder } from "../api"
import { Button } from "../components/Button"
import { Card } from "../components/Card"
import { Page } from "../components/Page"
import { Section } from "../components/Section"
import { StatusMessage } from "../components/StatusMessage"
import { useAppInfo, useAppUpdate } from "../queries"
import { strings } from "../strings"
import { AddingSection } from "./settings/AddingSection"
import { AppearanceSection } from "./settings/AppearanceSection"
import { BackgroundSection } from "./settings/BackgroundSection"
import { BackupRow } from "./settings/BackupRow"
import { CookiesSection, DownloadsSection, InstagramSection } from "./settings/DownloadsSection"
import { SupportedSitesSection } from "./settings/SupportedSitesSection"
import { ToolsSection } from "./settings/ToolsSection"

export function SettingsView() {
  return (
    <Page title={strings.nav.settings}>
      <div className="flex max-w-3xl flex-col gap-6">
        <DownloadsSection />
        <AddingSection />
        <BackgroundSection />
        <AppearanceSection />
        <InstagramSection />
        <CookiesSection />
        <SupportedSitesSection />
        <ToolsSection />
        <AboutSection />
      </div>
    </Page>
  )
}

/**
 * The version and Check for Updates, then the data folder and Open Folder. A newer release shows as the banner under
 * the add bar. A portable copy says it doesn't update itself instead.
 */
function AboutSection() {
  const { data: appInfo } = useAppInfo()
  const update = useAppUpdate()
  const checked = update.isFetchedAfterMount
  const [openFolderError, setOpenFolderError] = useState<string | null>(null)

  async function showDataFolder() {
    setOpenFolderError(null)
    try {
      await openDataFolder()
    } catch (caught) {
      setOpenFolderError(String(caught))
    }
  }

  return (
    <Section title={strings.settings.about}>
      <Card className="flex flex-col gap-3 px-4 py-3">
        <div className="flex items-center gap-3">
          <div className="min-w-0 flex-1">
            <p className="font-medium">{strings.appName}</p>
            {appInfo && (
              <p className="text-fg-secondary">
                {appInfo.portable ? strings.versionPortable(appInfo.version) : strings.version(appInfo.version)}
              </p>
            )}
          </div>
          {appInfo && !appInfo.portable && (
            <Button size="sm" loading={update.isFetching} onClick={() => void update.refetch()}>
              {strings.appUpdate.check}
            </Button>
          )}
        </div>
        {appInfo?.portable && <StatusMessage tone="neutral" message={strings.appUpdate.portable} />}
        {checked && !update.isFetching && update.isError && (
          <StatusMessage tone="danger" message={strings.appUpdate.checkFailed} detail={String(update.error)} />
        )}
        {checked && !update.isFetching && update.isSuccess && update.data === null && (
          <StatusMessage tone="success" message={strings.appUpdate.upToDate} />
        )}
        {checked && !update.isFetching && update.data && (
          <StatusMessage tone="neutral" message={strings.appUpdate.available(update.data.version)} />
        )}
        <div className="flex items-center gap-3 border-t border-line pt-3">
          <div className="min-w-0 flex-1">
            <p className="font-medium">{strings.settings.dataFolder}</p>
            <p className="text-fg-secondary">{strings.settings.dataFolderHint}</p>
            {appInfo && (
              <p className="truncate text-xs text-fg-muted select-text" title={appInfo.dataFolder}>
                {appInfo.dataFolder}
              </p>
            )}
          </div>
          <Button size="sm" onClick={() => void showDataFolder()}>
            {strings.settings.openDataFolder}
          </Button>
        </div>
        {openFolderError !== null && (
          <StatusMessage tone="danger" message={strings.settings.dataFolderOpenFailed} detail={openFolderError} />
        )}
        <BackupRow />
      </Card>
    </Section>
  )
}

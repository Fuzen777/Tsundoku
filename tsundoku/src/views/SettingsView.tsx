import { useEffect, useState, type ReactNode } from "react"
import { openDataFolder } from "../api"
import { useChapters, type Chapter } from "../chapters"
import { Button } from "../components/Button"
import { Card } from "../components/Card"
import { Page } from "../components/Page"
import { Section } from "../components/Section"
import { StatusMessage } from "../components/StatusMessage"
import {
  isSettingsSection,
  SETTINGS_SECTIONS,
  type SettingsSectionId,
  type SettingsTarget,
  useAppActions,
} from "../navigation"
import { useAppInfo, useAppUpdate } from "../queries"
import { scrollWithin } from "../scrollWithin"
import { strings } from "../strings"
import { AddingSection } from "./settings/AddingSection"
import { AppearanceSection } from "./settings/AppearanceSection"
import { ArchivesSection } from "./settings/ArchivesSection"
import { BackgroundSection } from "./settings/BackgroundSection"
import { PasswordsSection } from "./settings/PasswordsSection"
import { BackupRow } from "./settings/BackupRow"
import { CookiesSection, DownloadsSection, PacingSection, PostsSection } from "./settings/DownloadsSection"
import { ShortcutsSection } from "./settings/ShortcutsSection"
import { SupportedSitesSection } from "./settings/SupportedSitesSection"
import { ToolsSection } from "./settings/ToolsSection"

/** Settings' sections as the sidebar lists them under Settings. */
const SETTINGS_CHAPTERS: readonly Chapter[] = SETTINGS_SECTIONS.map(({ id, title }) => ({ id, title }))

/**
 * `place`: where Settings was asked to open, a section from another view or a control another view sent the user to
 * (the clipboard shortcut from the empty Downloads view's Set Up). The sidebar lists its sections under it, marking the
 * one at the top (`useChapters`).
 */
export function SettingsView({ place = null }: { place?: SettingsTarget | null }) {
  const { scrollRef, markShown, goTo } = useChapters({ view: "settings", chapters: SETTINGS_CHAPTERS })

  useEffect(() => {
    if (!place) return
    if (isSettingsSection(place.place)) {
      goTo(place.place)
      return
    }
    const target = document.getElementById(place.place)
    if (!target) return
    scrollWithin(target, "center")
    // The control itself (the clipboard shortcut's list), or the place's first button.
    const control = target.matches("select, button") ? target : target.querySelector<HTMLElement>("select, button")
    control?.focus({ preventScroll: true })
  }, [place, goTo])

  return (
    <Page title={strings.nav.settings} scrollRef={scrollRef} onScroll={markShown}>
      <div className="flex max-w-3xl flex-col gap-6">
        {SETTINGS_SECTIONS.map(({ id }) => {
          const Content = SECTION_CONTENTS[id]
          return (
            <div key={id} id={id} data-chapter={id} className="scroll-mt-4">
              <Content />
            </div>
          )
        })}
      </div>
    </Page>
  )
}

/** What each of Settings' sections shows; their order and titles are `SETTINGS_SECTIONS`'. */
const SECTION_CONTENTS: Record<SettingsSectionId, () => ReactNode> = {
  "settings-downloads": DownloadsSection,
  "settings-archives": ArchivesSection,
  "settings-passwords": PasswordsSection,
  "settings-adding": AddingSection,
  "settings-background": BackgroundSection,
  "settings-appearance": AppearanceSection,
  "settings-shortcuts": ShortcutsSection,
  "settings-posts": PostsSection,
  "settings-pacing": PacingSection,
  "settings-cookies": CookiesSection,
  "settings-sites": SupportedSitesSection,
  "settings-tools": ToolsSection,
  "settings-about": AboutSection,
}

/**
 * The version with What's New, which opens the What's New card on this version's changes, and Check for Updates, then
 * the data folder and Open Folder. A newer release shows as the banner under the add bar. A portable copy says it
 * doesn't update itself instead.
 */
function AboutSection() {
  const { showWhatsNew } = useAppActions()
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
          <Button size="sm" onClick={showWhatsNew}>
            {strings.whatsNew.open}
          </Button>
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

import { useCallback, useEffect, useRef, useState, type ReactNode } from "react"
import { openDataFolder } from "../api"
import { Button } from "../components/Button"
import { Card } from "../components/Card"
import { Page } from "../components/Page"
import { Section } from "../components/Section"
import { StatusMessage } from "../components/StatusMessage"
import { isSettingsSection, SETTINGS_SECTIONS, type SettingsSectionId, type SettingsTarget } from "../navigation"
import { useAppInfo, useAppUpdate } from "../queries"
import { scrollWithin } from "../scrollWithin"
import { strings } from "../strings"
import { AddingSection } from "./settings/AddingSection"
import { AppearanceSection } from "./settings/AppearanceSection"
import { ArchivesSection } from "./settings/ArchivesSection"
import { BackgroundSection } from "./settings/BackgroundSection"
import { BackupRow } from "./settings/BackupRow"
import { CookiesSection, DownloadsSection, PacingSection, PostsSection } from "./settings/DownloadsSection"
import { ShortcutsSection } from "./settings/ShortcutsSection"
import { SupportedSitesSection } from "./settings/SupportedSitesSection"
import { ToolsSection } from "./settings/ToolsSection"

/**
 * `place`: where Settings was asked to open, a section from the sidebar or a control another view sent the user to (the
 * clipboard shortcut from the empty queue's Set Up). `onSectionShown` hears which section is at the top as it scrolls,
 * for the sidebar to mark.
 */
export function SettingsView({
  place = null,
  onSectionShown,
}: {
  place?: SettingsTarget | null
  onSectionShown?: (section: SettingsSectionId) => void
}) {
  const scrollArea = useRef<HTMLDivElement>(null)
  // The section the sidebar last asked for: marked, rather than the one at the top, until the user scrolls themselves,
  // since the end of the page can keep it from reaching the top (Tools or About under a short window).
  const asked = useRef<SettingsSectionId | null>(null)
  const markShownSection = useCallback(() => {
    const area = scrollArea.current
    if (!area) return
    if (asked.current) return onSectionShown?.(asked.current)
    const areaBox = area.getBoundingClientRect()
    const topOf = (id: string) => document.getElementById(id)?.getBoundingClientRect().top ?? Infinity
    // The last section whose heading has reached the top, else the first.
    let shown: SettingsSectionId = SETTINGS_SECTIONS[0].id
    for (const { id } of SETTINGS_SECTIONS) if (topOf(id) <= areaBox.top + SECTION_REACHED_TOP) shown = id
    onSectionShown?.(shown)
  }, [onSectionShown])

  useEffect(() => {
    if (!place) return
    const target = document.getElementById(place.place)
    if (!target) return
    if (isSettingsSection(place.place)) {
      asked.current = place.place
      scrollWithin(target, "start", "smooth")
      markShownSection()
      return
    }
    scrollWithin(target, "center")
    // The control itself (the clipboard shortcut's list), or the place's first button.
    const control = target.matches("select, button") ? target : target.querySelector<HTMLElement>("select, button")
    control?.focus({ preventScroll: true })
  }, [place, markShownSection])

  useEffect(markShownSection, [markShownSection])

  // Scrolling by hand (the wheel, a drag on the scrollbar, the keyboard) goes back to marking the section at the top.
  useEffect(() => {
    const area = scrollArea.current
    if (!area) return
    const scrolledByHand = () => {
      asked.current = null
    }
    const inputs = ["wheel", "pointerdown", "keydown", "touchstart"] as const
    for (const input of inputs) area.addEventListener(input, scrolledByHand, { passive: true })
    return () => {
      for (const input of inputs) area.removeEventListener(input, scrolledByHand)
    }
  }, [])

  return (
    <Page title={strings.nav.settings} scrollRef={scrollArea} onScroll={markShownSection}>
      <div className="flex max-w-3xl flex-col gap-6">
        {SETTINGS_SECTIONS.map(({ id }) => {
          const Content = SECTION_CONTENTS[id]
          return (
            <div key={id} id={id} className="scroll-mt-4">
              <Content />
            </div>
          )
        })}
      </div>
    </Page>
  )
}

/** How far below the top of the scrolling area a section's heading counts as having reached it, in pixels. */
const SECTION_REACHED_TOP = 48

/** What each of Settings' sections shows; their order and titles are `SETTINGS_SECTIONS`'. */
const SECTION_CONTENTS: Record<SettingsSectionId, () => ReactNode> = {
  "settings-downloads": DownloadsSection,
  "settings-archives": ArchivesSection,
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

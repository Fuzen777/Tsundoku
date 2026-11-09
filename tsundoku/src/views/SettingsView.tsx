import { useEffect, useState, type ReactNode } from "react"
import { openDataFolder } from "../api"
import { Button } from "../components/Button"
import { Card } from "../components/Card"
import { Page } from "../components/Page"
import { Section } from "../components/Section"
import { StatusMessage } from "../components/StatusMessage"
import type { SettingsTarget } from "../navigation"
import { useAppInfo, useAppUpdate } from "../queries"
import { strings } from "../strings"
import { AddingSection } from "./settings/AddingSection"
import { AppearanceSection } from "./settings/AppearanceSection"
import { BackgroundSection } from "./settings/BackgroundSection"
import { BackupRow } from "./settings/BackupRow"
import { CookiesSection, DownloadsSection, InstagramSection, PostsSection } from "./settings/DownloadsSection"
import { ShortcutsSection } from "./settings/ShortcutsSection"
import { SupportedSitesSection } from "./settings/SupportedSitesSection"
import { ToolsSection } from "./settings/ToolsSection"

/** `place`: where another view sent the user, such as the clipboard shortcut from the empty queue's Set Up. */
export function SettingsView({ place = null }: { place?: SettingsTarget | null }) {
  useEffect(() => {
    if (!place) return
    const target = document.getElementById(place.place)
    target?.scrollIntoView({ block: "center" })
    // The control itself (the clipboard shortcut's list), or the place's first button.
    const control = target?.matches("select, button") ? target : target?.querySelector<HTMLElement>("select, button")
    control?.focus({ preventScroll: true })
  }, [place])

  return (
    <Page title={strings.nav.settings}>
      <div className="flex max-w-3xl flex-col gap-6">
        <nav aria-label={strings.settings.jumpTo} className="-mt-1 flex flex-wrap gap-1.5">
          {SETTINGS_SECTIONS.map(({ id, title }) => (
            <button
              key={id}
              type="button"
              onClick={() => document.getElementById(id)?.scrollIntoView({ block: "start", behavior: "smooth" })}
              className="rounded-full border border-line px-2.5 py-0.5 text-xs font-medium text-fg-secondary transition-colors hover:border-line-strong hover:text-fg focus-visible:outline-2 focus-visible:outline-accent"
            >
              {title}
            </button>
          ))}
        </nav>
        {SETTINGS_SECTIONS.map(({ id, Content }) => (
          <div key={id} id={id} className="scroll-mt-4">
            <Content />
          </div>
        ))}
      </div>
    </Page>
  )
}

/** Settings' sections in order, with the title each jump link at the top shows (the section's own heading). */
const SETTINGS_SECTIONS: readonly { id: string; title: string; Content: () => ReactNode }[] = [
  { id: "settings-downloads", title: strings.settings.downloads, Content: DownloadsSection },
  { id: "settings-adding", title: strings.settings.adding, Content: AddingSection },
  { id: "settings-background", title: strings.settings.background, Content: BackgroundSection },
  { id: "settings-appearance", title: strings.settings.appearance, Content: AppearanceSection },
  { id: "settings-shortcuts", title: strings.settings.keyboardShortcuts, Content: ShortcutsSection },
  { id: "settings-posts", title: strings.settings.posts, Content: PostsSection },
  { id: "settings-instagram", title: strings.settings.instagram, Content: InstagramSection },
  { id: "settings-cookies", title: strings.settings.cookies, Content: CookiesSection },
  { id: "settings-sites", title: strings.settings.sites.title, Content: SupportedSitesSection },
  { id: "settings-tools", title: strings.settings.tools, Content: ToolsSection },
  { id: "settings-about", title: strings.settings.about, Content: AboutSection },
]

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

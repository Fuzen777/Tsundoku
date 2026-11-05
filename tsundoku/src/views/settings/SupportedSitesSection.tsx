import { useDeferredValue, useState, type ReactNode } from "react"
import type { SupportedSite } from "../../bindings/SupportedSite"
import { Card } from "../../components/Card"
import { TextInput } from "../../components/Field"
import { Section } from "../../components/Section"
import { StatusMessage } from "../../components/StatusMessage"
import { describeToolError } from "../../errors"
import { useSupportedSites } from "../../queries"
import { strings } from "../../strings"

const text = strings.settings.sites

/**
 * Settings → Supported Sites: every site Tsundoku can read, as one list whatever tool reads it. Sites that need an
 * account come first, with the browser profiles signed in to them, so it's clear which sign-ins Tsundoku will use.
 */
export function SupportedSitesSection() {
  const { data, error, isPending } = useSupportedSites()
  const [search, setSearch] = useState("")
  const query = useDeferredValue(search.trim().toLowerCase())
  const matches = (site: SupportedSite) => site.name.toLowerCase().includes(query)
  const sites = data?.sites.filter(matches) ?? []
  const accountSites = sites.filter((site) => site.signedIn !== null)
  const signedIn = accountSites.filter((site) => site.signedIn?.length)
  const notSignedIn = accountSites.filter((site) => !site.signedIn?.length)
  const others = sites.filter((site) => site.signedIn === null)

  return (
    <Section title={text.title}>
      <Card className="flex flex-col gap-4 p-4">
        <p className="text-xs text-fg-secondary">{text.intro}</p>
        {isPending && <p className="text-sm text-fg-muted">{text.loading}</p>}
        {error && <StatusMessage tone="danger" {...describeToolError(error, "ytDlp")} />}
        {data && (
          <>
            <TextInput
              aria-label={text.search}
              placeholder={text.search}
              value={search}
              onChange={(event) => setSearch(event.target.value)}
            />
            {signedIn.length > 0 && (
              <SiteGroup title={text.signedIn}>
                {signedIn.map((site) => (
                  <AccountRow key={site.name} site={site} />
                ))}
              </SiteGroup>
            )}
            {notSignedIn.length > 0 && (
              <SiteGroup title={text.notSignedIn}>
                {notSignedIn.map((site) => (
                  <AccountRow key={site.name} site={site} />
                ))}
              </SiteGroup>
            )}
            <SiteGroup title={text.allSites(others.length)}>
              {others.length === 0 ? (
                <p className="text-sm text-fg-muted">{text.noMatches}</p>
              ) : (
                <ul className="grid max-h-72 grid-cols-[repeat(auto-fill,minmax(10rem,1fr))] gap-x-4 gap-y-1 overflow-y-auto text-sm">
                  {others.map((site) => (
                    <li key={site.name} className="truncate" title={site.name}>
                      {site.name}
                    </li>
                  ))}
                </ul>
              )}
            </SiteGroup>
            {!data.imagesSetUp && <p className="text-xs text-fg-muted">{text.imagesNotSetUp}</p>}
          </>
        )}
      </Card>
    </Section>
  )
}

function SiteGroup({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-2">
      <h3 className="text-xs font-semibold text-fg-secondary">{title}</h3>
      {children}
    </div>
  )
}

/** A site that needs an account, and the browser profiles signed in to it, the one Tsundoku uses first. */
function AccountRow({ site }: { site: SupportedSite }) {
  const profiles = site.signedIn ?? []
  return (
    <div className="flex items-baseline justify-between gap-4 text-sm">
      <span>{site.name}</span>
      <span className={profiles.length ? "text-success text-right" : "text-fg-muted text-right"}>
        {profiles.length
          ? profiles
              .map((profile) => text.profile(strings.settings.browsers[profile.browser], profile.profile))
              .join(", ")
          : text.signInHint(site.name)}
      </span>
    </div>
  )
}

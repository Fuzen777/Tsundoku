import { useState, type ReactNode } from "react"
import { siteIconUrl } from "../api"
import { classes } from "./styles"

interface SiteIconProps {
  /** A page on the site, whose host names the icon. */
  page: string
  /** The site as people know it ("youtube.com"): the icon's hover text, and what screen readers say. */
  site: string
  /** Shown in its place while there's none: in a browser, or for a site without one. */
  fallback?: ReactNode
  className?: string
}

/** The icon of the site a page is on (its favicon, read once by the core and kept: `siteIconUrl`), 16 px. */
export function SiteIcon({ page, site, fallback = null, className }: SiteIconProps) {
  const url = siteIconUrl(page)
  const [failed, setFailed] = useState<string | null>(null)
  if (!url || failed === url) return fallback
  return (
    <img
      src={url}
      alt={site}
      title={site}
      className={classes("size-4 shrink-0 rounded-sm object-contain", className)}
      onError={() => setFailed(url)}
    />
  )
}

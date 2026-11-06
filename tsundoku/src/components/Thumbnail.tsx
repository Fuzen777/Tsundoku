import { useState } from "react"
import { classes } from "./styles"

interface ThumbnailProps {
  url: string | null
  /** Shown when `url` doesn't load, such as the site's picture when the thumbnail kept in the data folder is gone. */
  fallbackUrl?: string | null
  className: string
}

/**
 * A video's thumbnail in a 16:9 tile: `url`, else `fallbackUrl`, else the empty tile when it has none or neither
 * loads. Its width comes from `className`. Give it `key={url}`, so a thumbnail that failed doesn't hide the next
 * video's.
 */
export function Thumbnail({ url, fallbackUrl = null, className }: ThumbnailProps) {
  const sources = [url, fallbackUrl].filter((source): source is string => source !== null)
  const [failures, setFailures] = useState(0)
  const source = sources.at(failures)
  return (
    <div className={classes("aspect-video shrink-0 overflow-hidden rounded-md bg-input", className)}>
      {source && (
        <img
          key={source}
          src={source}
          alt=""
          loading="lazy"
          referrerPolicy="no-referrer"
          className="size-full object-cover"
          onError={() => setFailures((count) => count + 1)}
        />
      )}
    </div>
  )
}

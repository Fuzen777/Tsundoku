import { useState } from "react"
import { classes } from "./styles"

/**
 * A video's thumbnail in a 16:9 tile, or the empty tile when it has none or it doesn't load. Its width comes from
 * `className`. Give it `key={url}`, so a thumbnail that failed doesn't hide the next video's.
 */
export function Thumbnail({ url, className }: { url: string | null; className: string }) {
  const [failed, setFailed] = useState(false)
  return (
    <div className={classes("aspect-video shrink-0 overflow-hidden rounded-md bg-input", className)}>
      {url && !failed && (
        <img
          src={url}
          alt=""
          loading="lazy"
          referrerPolicy="no-referrer"
          className="size-full object-cover"
          onError={() => setFailed(true)}
        />
      )}
    </div>
  )
}

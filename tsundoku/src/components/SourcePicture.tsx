import { useState } from "react"
import { sitePictureUrl, watchedPictureUrl } from "../api"
import type { ListingKind } from "../bindings/ListingKind"
import type { PictureCrop } from "../bindings/PictureCrop"
import type { SourcePicture as PickedPicture } from "../bindings/SourcePicture"
import { cropPlacement } from "../pictureCrop"
import { useWatchedSources } from "../queries"
import { ListingPicture } from "./ListingPicker"
import { classes } from "./styles"

interface SourcePictureProps {
  /** The channel, playlist or account, with its listing's own picture. */
  listing: { kind: ListingKind; thumbnailUrl: string | null }
  /** The picture picked for it by hand, if any, shown instead. */
  picture: PickedPicture | null
}

/** A watched source's picture, small: the one picked for it, cropped, else its listing's own (`ListingPicture`). The
 * listing's shows too when the picked one doesn't load, or in a browser, where it can't be read. */
export function SourcePicture({ listing, picture }: SourcePictureProps) {
  const url = picture && watchedPictureUrl(picture.file)
  const [failed, setFailed] = useState<string | null>(null)
  if (!picture || !url || failed === url) return <ListingPicture listing={listing} size="small" />
  return <CroppedPicture kind={listing.kind} url={url} crop={picture.crop} onError={() => setFailed(url)} />
}

/** A channel's, playlist's or account's picture in the Downloads view, which shows the picture picked for it when it's
 * watched. */
export function QueueSourcePicture({ listing }: { listing: { kind: ListingKind; thumbnailUrl: string | null; url: string } }) {
  const { data: sources } = useWatchedSources()
  const picture = sources?.find((source) => source.url === listing.url)?.picture ?? null
  return <SourcePicture listing={listing} picture={picture} />
}

interface CroppedPictureProps {
  kind: ListingKind
  url: string
  crop: PictureCrop
  onError?: () => void
}

/** `crop` of a picture in a source's small frame: a 56 px circle, or a 96 px 16:9 thumbnail for a playlist. */
export function CroppedPicture({ kind, url, crop, onError }: CroppedPictureProps) {
  return (
    <div
      className={classes(
        "relative shrink-0 overflow-hidden bg-input",
        kind === "playlist" ? "aspect-video w-24 rounded-md" : "size-14 rounded-full",
      )}
    >
      <img
        src={sitePictureUrl(url)}
        alt=""
        referrerPolicy="no-referrer"
        draggable={false}
        className="absolute max-w-none"
        style={cropPlacement(crop)}
        onError={onError}
      />
    </div>
  )
}

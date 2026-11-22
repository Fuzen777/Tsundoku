import { cropWatchedPicture, pickWatchedPicture, resetWatchedPicture, watchedPictureUrl } from "./api"
import type { PictureCrop } from "./bindings/PictureCrop"
import type { WatchedSource } from "./bindings/WatchedSource"
import { sameCrop } from "./pictureCrop"

/** A watched source's picture while Edit is open, saved with its other choices. */
export type PictureDraft =
  | { kind: "automatic" }
  | {
      kind: "custom"
      /** The picture shown: one of the source's, or the one picked before (`stored`); null until one is picked. */
      url: string | null
      /** It's the picture picked before, kept in the data folder, so only its area can change. */
      stored: boolean
      /** The area of it shown; null until the picture has loaded. */
      crop: PictureCrop | null
    }

/** The source's picture as it is: Custom with the one picked before, else Automatic. */
export function pictureDraftOf(source: WatchedSource): PictureDraft {
  if (!source.picture) return { kind: "automatic" }
  return { kind: "custom", url: watchedPictureUrl(source.picture.file), stored: true, crop: source.picture.crop }
}

/** Custom, once picked: the picture picked before, else the listing's own to start with. */
export function customDraftOf(source: WatchedSource): PictureDraft {
  if (source.picture) return pictureDraftOf(source)
  return { kind: "custom", url: source.thumbnailUrl, stored: false, crop: null }
}

/** One of the source's pictures, picked in the list, shown whole to start with. */
export function pickedDraft(url: string): PictureDraft {
  return { kind: "custom", url, stored: false, crop: null }
}

/** Saves what changed: Automatic again, another area of the picture picked before, or another picture. Custom without
 * a picture that has loaded changes nothing. */
export async function savePictureDraft(source: WatchedSource, draft: PictureDraft): Promise<void> {
  if (draft.kind === "automatic") {
    if (source.picture) await resetWatchedPicture(source.id)
    return
  }
  if (!draft.url || !draft.crop) return
  if (!draft.stored) {
    await pickWatchedPicture(source.id, draft.url, draft.crop)
  } else if (source.picture && !sameCrop(source.picture.crop, draft.crop)) {
    await cropWatchedPicture(source.id, draft.crop)
  }
}

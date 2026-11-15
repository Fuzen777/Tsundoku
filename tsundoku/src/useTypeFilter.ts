import { useState } from "react"
import type { VideoInfo } from "./bindings/VideoInfo"
import { ofType, typeCounts, typeInEffect, type TypeFilter } from "./listingPicker"

/**
 * A picker's type filter over `items`: how many are of each type, the type in effect (the one chosen, while there are
 * several types and some of it, else all) and the items it shows. The choice lasts while the picker is open.
 */
export function useTypeFilter<Item extends { video: Pick<VideoInfo, "post" | "files"> }>(items: readonly Item[]) {
  const [chosen, choose] = useState<TypeFilter>("all")
  const counts = typeCounts(items)
  const type = typeInEffect(chosen, counts)
  return { counts, type, choose, shown: ofType(items, type) }
}

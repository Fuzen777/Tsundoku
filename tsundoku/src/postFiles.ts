import type { PostFile } from "./bindings/PostFile"
import type { VideoInfo } from "./bindings/VideoInfo"
import { strings } from "./strings"

/** The files ticked to start with: those its downloads left out, when they left some out, else every one. */
export function defaultPick(files: readonly PostFile[], leftOut: readonly number[] | null | undefined): Set<number> {
  return new Set(leftOut && leftOut.length > 0 ? leftOut : files.map((file) => file.number))
}

/** The files a post's download takes now, as its files say (`PostFile.picked`). */
export function pickedOf(files: readonly PostFile[]): Set<number> {
  return new Set(files.filter((file) => file.picked).map((file) => file.number))
}

/** The post with only the `picked` files taken, to download. */
export function withPicked(video: VideoInfo, picked: ReadonlySet<number>): VideoInfo {
  return { ...video, postFiles: video.postFiles?.map((file) => ({ ...file, picked: picked.has(file.number) })) }
}

/** "3 of 6 files" for a post whose download leaves some files out; null when it takes them all, or isn't a post. */
export function pickSummary(video: VideoInfo): string | null {
  const files = video.postFiles ?? []
  const picked = files.filter((file) => file.picked).length
  return picked < files.length ? strings.postFiles.summary(picked, files.length) : null
}

import type { PostFile } from "./bindings/PostFile"
import type { VideoInfo } from "./bindings/VideoInfo"
import { sameFiles, sharedPickSummary, type ListedFile } from "./sharedFiles"
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

/** Whether a post's files show as a list of their names and sizes rather than a strip of pictures: when some of them
 * aren't images or videos, as in a folder on Gofile or Pixeldrain. */
export function listsPostFiles(video: VideoInfo): boolean {
  return (video.post?.others ?? 0) > 0
}

/** A post's files as the list shows them, flat and in the post's order: each by its name (else "File 3"), with its size
 * when the site gave it. A "/" in a name, which no saved file can have, isn't read as a folder. */
export function postListFiles(files: readonly PostFile[]): ListedFile[] {
  return files.map((file) => ({
    path: (file.name ?? strings.postFiles.namelessFile(file.number)).replaceAll("/", "_"),
    size: file.size ?? null,
    number: file.number,
  }))
}

/** What a post downloaded before says over its files: how many weren't picked then, and, while they're the ones ticked,
 * that downloading again gets just those. */
export function postLeftOutMessage(leftOut: readonly number[], picked: ReadonlySet<number>): string {
  return sameFiles(new Set(leftOut), picked)
    ? strings.postFiles.leftOut(leftOut.length)
    : strings.postFiles.leftOutCount(leftOut.length)
}

/** "3 of 6 files" for a post whose download leaves some files out, "12 of 340 files, 1.2 GB" for a shared folder
 * (`sharedPickSummary`); null when it takes them all, or is neither. */
export function pickSummary(video: VideoInfo): string | null {
  const files = video.postFiles ?? []
  const picked = files.filter((file) => file.picked).length
  return picked < files.length ? strings.postFiles.summary(picked, files.length) : sharedPickSummary(video)
}

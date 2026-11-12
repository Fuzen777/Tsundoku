import type { SharedFile } from "./bindings/SharedFile"
import type { VideoInfo } from "./bindings/VideoInfo"
import { formatBytes } from "./format"
import { strings } from "./strings"

/** Folders start folded in a list of more files than this, so a big folder opens on its outline. */
export const FOLDED_ABOVE = 200

/** One folder of a shared folder's list, with what's in it in the order the host gave it. */
export interface SharedFolder {
  /** Its path from the list's top, joined with "/": the key it's folded by. "" for the top. */
  path: string
  name: string
  folders: SharedFolder[]
  /** Its own files, by their place in the link's list (`VideoInfo.sharedFiles`). */
  files: number[]
  /** Every file under it, in its folders too. */
  allFiles: number[]
  /** Their size together. */
  bytes: number
}

/** A line of the list as it shows: a folder, open or folded, or a file, `depth` levels in. */
export type SharedRow =
  | { kind: "folder"; folder: SharedFolder; depth: number; open: boolean }
  | { kind: "file"; index: number; name: string; depth: number }

/** A file's path split into its folders and name, without the folder every file is in, when there's one. */
function partsOf(files: readonly SharedFile[]): string[][] {
  const split = files.map((file) => file.path.split("/").filter((part) => part !== ""))
  const top = split[0]?.[0]
  // Every host puts the shared folder's own name first, which the card's title already says: the list starts inside it.
  const oneTop = top !== undefined && split.every((parts) => parts.length > 1 && parts[0] === top)
  return oneTop ? split.map((parts) => parts.slice(1)) : split
}

/** The path a file shows at in the list, from its top: what the filter matches. */
export function shownPaths(files: readonly SharedFile[]): string[] {
  return partsOf(files).map((parts) => parts.join("/"))
}

/** The folders and files of a shared folder's list. Each folder holds its folders first, then its files, each in the
 * order the host gave them. */
export function sharedTree(files: readonly SharedFile[]): SharedFolder {
  const top: SharedFolder = { path: "", name: "", folders: [], files: [], allFiles: [], bytes: 0 }
  const byPath = new Map<string, SharedFolder>([["", top]])
  partsOf(files).forEach((parts, index) => {
    const size = files[index].size
    let folder = top
    folder.allFiles.push(index)
    folder.bytes += size
    for (const name of parts.slice(0, -1)) {
      const path = folder.path === "" ? name : `${folder.path}/${name}`
      let inner = byPath.get(path)
      if (!inner) {
        inner = { path, name, folders: [], files: [], allFiles: [], bytes: 0 }
        byPath.set(path, inner)
        folder.folders.push(inner)
      }
      folder = inner
      folder.allFiles.push(index)
      folder.bytes += size
    }
    folder.files.push(index)
  })
  return top
}

/** Every folder's path under `folder`, for folding them all. */
export function folderPaths(folder: SharedFolder): string[] {
  return folder.folders.flatMap((inner) => [inner.path, ...folderPaths(inner)])
}

/** The files whose path from the list's top holds `filter`, ignoring case; null when there's nothing to filter by. */
export function matchingFiles(paths: readonly string[], filter: string): Set<number> | null {
  const wanted = filter.trim().toLowerCase()
  if (wanted === "") return null
  const matching = new Set<number>()
  paths.forEach((path, index) => {
    if (path.toLowerCase().includes(wanted)) matching.add(index)
  })
  return matching
}

/** The lines the list shows: open folders' contents under them, folded ones without. While filtering (`matching`),
 * only the files that match, under every folder that holds one, all open. */
export function visibleRows(
  top: SharedFolder,
  files: readonly SharedFile[],
  folded: ReadonlySet<string>,
  matching: ReadonlySet<number> | null,
): SharedRow[] {
  const rows: SharedRow[] = []
  const nameOf = (index: number) => files[index].path.slice(files[index].path.lastIndexOf("/") + 1)
  const walk = (folder: SharedFolder, depth: number) => {
    for (const inner of folder.folders) {
      if (matching && !inner.allFiles.some((index) => matching.has(index))) continue
      const open = matching !== null || !folded.has(inner.path)
      rows.push({ kind: "folder", folder: inner, depth, open })
      if (open) walk(inner, depth + 1)
    }
    for (const index of folder.files) {
      if (matching && !matching.has(index)) continue
      rows.push({ kind: "file", index, name: nameOf(index), depth })
    }
  }
  walk(top, 0)
  return rows
}

/** Whether a folder's files are all ticked, some, or none: only those `matching`, while filtering. */
export function folderState(
  folder: SharedFolder,
  picked: ReadonlySet<number>,
  matching: ReadonlySet<number> | null,
): "all" | "some" | "none" {
  const files = matching ? folder.allFiles.filter((index) => matching.has(index)) : folder.allFiles
  const ticked = files.filter((index) => picked.has(index)).length
  return ticked === 0 ? "none" : ticked === files.length ? "all" : "some"
}

/** `picked` with `files` ticked or unticked. */
export function withFiles(picked: ReadonlySet<number>, files: Iterable<number>, on: boolean): Set<number> {
  const next = new Set(picked)
  for (const index of files) {
    if (on) next.add(index)
    else next.delete(index)
  }
  return next
}

/** The files ticked to start with: those its downloads left out, when they left some out, else every one. */
export function defaultSharedPick(files: readonly SharedFile[], leftOut: readonly number[] | null | undefined): Set<number> {
  return new Set(leftOut && leftOut.length > 0 ? leftOut : files.map((_, index) => index))
}

/** The files a shared folder's download takes now, as its files say (`SharedFile.picked`). */
export function sharedPickedOf(files: readonly SharedFile[]): Set<number> {
  const picked = new Set<number>()
  files.forEach((file, index) => {
    if (file.picked) picked.add(index)
  })
  return picked
}

/** The shared folder with only the `picked` files taken, to download. */
export function withSharedPicked(video: VideoInfo, picked: ReadonlySet<number>): VideoInfo {
  return { ...video, sharedFiles: video.sharedFiles?.map((file, index) => ({ ...file, picked: picked.has(index) })) }
}

/** Whether two picks tick the same files. */
export function sameFiles(first: ReadonlySet<number>, second: ReadonlySet<number>): boolean {
  if (first.size !== second.size) return false
  for (const index of first) if (!second.has(index)) return false
  return true
}

/** The size of the `picked` files together. */
export function pickedBytes(files: readonly SharedFile[], picked: ReadonlySet<number>): number {
  let bytes = 0
  for (const index of picked) bytes += files[index]?.size ?? 0
  return bytes
}

/** "12 of 340 files, 1.2 GB" for a shared folder whose download leaves some files out; null when it takes them all,
 * or the link lists no files. */
export function sharedPickSummary(video: VideoInfo): string | null {
  const files = video.sharedFiles ?? []
  const picked = sharedPickedOf(files)
  return picked.size < files.length
    ? strings.sharedFiles.summary(picked.size, files.length, formatBytes(pickedBytes(files, picked)))
    : null
}

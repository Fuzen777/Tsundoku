import { useMemo } from "react"
import { postListFiles } from "../postFiles"
import { PostFilesStrip, type PostFilesStripProps } from "./PostFilesStrip"
import { SharedFilesList } from "./SharedFilesList"

/**
 * A post's files to pick from: the strip of pictures, or, for a post of files that aren't all images or videos (`asList`:
 * a folder on Gofile or Pixeldrain), the list of their names and sizes.
 */
export function PostFilesPicker({ asList, ...props }: PostFilesStripProps & { asList: boolean }) {
  return asList ? <PostFilesList {...props} /> : <PostFilesStrip {...props} />
}

/** A post's files in the shared folder's list, flat: its picks and marks go by the files' numbers, as the strip's do,
 * and the list's by their places. */
function PostFilesList({ files, picked, leftOut, downloadedBefore, disabled, onChange }: PostFilesStripProps) {
  const listed = useMemo(() => postListFiles(files), [files])
  const placesOf = (numbers: Iterable<number>) => {
    const wanted = new Set(numbers)
    return files.flatMap((file, place) => (wanted.has(file.number) ? [place] : []))
  }
  return (
    <SharedFilesList
      files={listed}
      picked={new Set(placesOf(picked))}
      leftOut={leftOut && placesOf(leftOut)}
      downloadedBefore={downloadedBefore}
      ofPost
      disabled={disabled}
      onChange={(places) => onChange(new Set([...places].map((place) => files[place].number)))}
    />
  )
}

import { checkLinkFile, linksInLinkFile } from "./api"
import { describeLinkFileError, type ErrorDescription } from "./errors"
import { strings } from "./strings"

/** Over this many links, adding a dropped file's asks first, since checking them all takes a while. */
export const ASK_BEFORE_ADDING_OVER = 300

/** About how long checking a link takes: the tools start and answer, even for a site they turn down. */
const SECONDS_PER_LINK = 2.5

/** About how long checking this many links takes, in words: "about 25 minutes", "about 1.5 hours". */
export function checkingTime(links: number): string {
  const minutes = Math.max(1, Math.round((links * SECONDS_PER_LINK) / 60))
  if (minutes < 60) return strings.addBar.files.aboutMinutes(minutes)
  return strings.addBar.files.aboutHours(Math.round(minutes / 30) / 2)
}

export interface LinkFilesRead {
  /** Every link the files held, each once, in order. */
  links: string[]
  /** How many files were of a kind Tsundoku reads links from, whether or not they held any. */
  readable: number
  /** The names of the files that held links. */
  sources: string[]
  /** Why files gave no links, each reason once (the names of files refused for the same reason are listed together). */
  problems: ErrorDescription[]
}

/**
 * Reads the links in files dropped on the window, one after another: the core checks each by its name and size
 * before its bytes are read, then reads its links from them (a text file, a bookmarks export or an Internet
 * shortcut).
 */
export async function readLinkFiles(files: readonly File[]): Promise<LinkFilesRead> {
  const links = new Set<string>()
  const problems: ErrorDescription[] = []
  let readable = 0
  const sources: string[] = []
  for (const file of files) {
    try {
      await checkLinkFile(file.name, file.size)
      readable += 1
      const bytes = new Uint8Array(await file.arrayBuffer())
      for (const link of await linksInLinkFile(file.name, bytes)) links.add(link)
      sources.push(file.name)
    } catch (error) {
      const problem = describeLinkFileError(error, file.name)
      const same = problems.find((known) => known.message === problem.message)
      if (!same) problems.push(problem)
      else if (problem.detail) same.detail = same.detail ? `${same.detail}, ${problem.detail}` : problem.detail
    }
  }
  return { links: [...links], readable, sources, problems }
}

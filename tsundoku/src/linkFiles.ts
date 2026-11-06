import { checkLinkFile, linksInFileAt, linksInLinkFile, pickLinkFiles } from "./api"
import type { LinkFileError } from "./bindings/LinkFileError"
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

/** A file to read links from: its name, and how its links are read. */
interface LinkSource {
  name: string
  read: () => Promise<string[]>
}

/** The files the file dialog offers: those the core reads links from (`check_link_file`), which decides anyway. */
const LINK_FILE_EXTENSIONS = ["txt", "html", "htm", "url"]

/**
 * Reads the links in files dropped on the window, one after another: the core checks each by its name and size
 * before its bytes are read, then reads its links from them (a text file, a bookmarks export or an Internet
 * shortcut).
 */
export function readLinkFiles(files: readonly File[]): Promise<LinkFilesRead> {
  return readSources(
    files.map((file) => ({
      name: file.name,
      read: async () => {
        await checkLinkFile(file.name, file.size)
        return linksInLinkFile(file.name, new Uint8Array(await file.arrayBuffer()))
      },
    })),
  )
}

/** Asks for files of links on this PC (Choose File… in the empty views), and reads them, by the core: none when the
 * dialog is cancelled. */
export async function chooseLinkFiles(): Promise<LinkFilesRead | null> {
  const text = strings.addBar.files
  const paths = await pickLinkFiles(text.chooseTitle, text.chooseFilter, LINK_FILE_EXTENSIONS)
  if (paths.length === 0) return null
  return readSources(paths.map((path) => ({ name: path.split(/[\\/]/).pop() ?? path, read: () => linksInFileAt(path) })))
}

async function readSources(files: readonly LinkSource[]): Promise<LinkFilesRead> {
  const links = new Set<string>()
  const problems: ErrorDescription[] = []
  let readable = 0
  const sources: string[] = []
  for (const file of files) {
    try {
      for (const link of await file.read()) links.add(link)
      readable += 1
      sources.push(file.name)
    } catch (error) {
      if ((error as Partial<LinkFileError> | null)?.kind !== "unsupportedType") readable += 1
      const problem = describeLinkFileError(error, file.name)
      const same = problems.find((known) => known.message === problem.message)
      if (!same) problems.push(problem)
      else if (problem.detail) same.detail = same.detail ? `${same.detail}, ${problem.detail}` : problem.detail
    }
  }
  return { links: [...links], readable, sources, problems }
}

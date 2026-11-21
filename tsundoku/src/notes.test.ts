import { describe, expect, it } from "vitest"
import type { ChangelogSection } from "./bindings/ChangelogSection"
import { isWebLink, parseNotes, parseSpans, sectionDate, sectionToShow } from "./notes"

describe("parseNotes", () => {
  it("reads a CHANGELOG section's headings and lists", () => {
    const notes = [
      "### Added",
      "",
      "- **Back up and restore**: Settings → About's `Back Up…` saves them.",
      "- **Copy Details**: copies",
      "  what a bug report needs.",
      "",
      "### Fixed",
      "",
      "- **A bug**: gone.",
    ].join("\r\n")
    expect(parseNotes(notes)).toEqual([
      { kind: "heading", spans: [{ kind: "text", text: "Added" }] },
      {
        kind: "list",
        items: [
          [
            { kind: "bold", text: "Back up and restore" },
            { kind: "text", text: ": Settings → About's " },
            { kind: "code", text: "Back Up…" },
            { kind: "text", text: " saves them." },
          ],
          [
            { kind: "bold", text: "Copy Details" },
            { kind: "text", text: ": copies what a bug report needs." },
          ],
        ],
      },
      { kind: "heading", spans: [{ kind: "text", text: "Fixed" }] },
      { kind: "list", items: [[{ kind: "bold", text: "A bug" }, { kind: "text", text: ": gone." }]] },
    ])
  })

  it("joins a paragraph's lines and ends it at a blank line or a list", () => {
    expect(parseNotes("First line\nsecond line.\n\nAnother.\n* An item")).toEqual([
      { kind: "paragraph", spans: [{ kind: "text", text: "First line second line." }] },
      { kind: "paragraph", spans: [{ kind: "text", text: "Another." }] },
      { kind: "list", items: [[{ kind: "text", text: "An item" }]] },
    ])
  })

  it("has nothing to show for empty notes", () => {
    expect(parseNotes("")).toEqual([])
    expect(parseNotes("\n  \n")).toEqual([])
  })
})

describe("parseSpans", () => {
  it("reads links", () => {
    expect(parseSpans("See the [changelog](https://github.com/Fuzen777/Tsundoku) for more")).toEqual([
      { kind: "text", text: "See the " },
      { kind: "link", text: "changelog", url: "https://github.com/Fuzen777/Tsundoku" },
      { kind: "text", text: " for more" },
    ])
  })

  it("leaves a lone asterisk or backtick as it is", () => {
    expect(parseSpans("5 * 3 and a ` mark")).toEqual([{ kind: "text", text: "5 * 3 and a ` mark" }])
  })
})

describe("sectionToShow", () => {
  const section = (version: string): ChangelogSection => ({ version, date: null, body: "" })
  const sections = [section("0.2.1"), section("0.2.0"), section("0.1.0")]

  it("starts on the running version's changes", () => {
    expect(sectionToShow(sections, null, "0.2.0")?.version).toBe("0.2.0")
  })

  it("shows the version picked", () => {
    expect(sectionToShow(sections, "0.1.0", "0.2.1")?.version).toBe("0.1.0")
  })

  it("falls back to the newest when the running version has no section", () => {
    expect(sectionToShow(sections, null, "0.2.2")?.version).toBe("0.2.1")
    expect(sectionToShow(sections, "0.0.1", undefined)?.version).toBe("0.2.1")
  })

  it("has nothing to show without sections", () => {
    expect(sectionToShow([], null, "0.2.1")).toBeNull()
  })
})

describe("sectionDate", () => {
  it("writes a calendar date as the reader does", () => {
    expect(sectionDate({ version: "0.2.1", date: "2026-11-20", body: "" }, "en-US")).toBe("Nov 20, 2026")
  })

  it("leaves anything else as the changelog has it", () => {
    expect(sectionDate({ version: "0.2.1", date: "Unreleased", body: "" }, "en-US")).toBe("Unreleased")
    expect(sectionDate({ version: "0.2.1", date: null, body: "" }, "en-US")).toBeNull()
  })
})

describe("isWebLink", () => {
  it("opens only web pages", () => {
    expect(isWebLink("https://github.com")).toBe(true)
    expect(isWebLink("file:///C:/Windows")).toBe(false)
    expect(isWebLink("javascript:alert(1)")).toBe(false)
  })
})

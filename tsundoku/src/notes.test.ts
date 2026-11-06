import { describe, expect, it } from "vitest"
import { isWebLink, parseNotes, parseSpans } from "./notes"

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

describe("isWebLink", () => {
  it("opens only web pages", () => {
    expect(isWebLink("https://github.com")).toBe(true)
    expect(isWebLink("file:///C:/Windows")).toBe(false)
    expect(isWebLink("javascript:alert(1)")).toBe(false)
  })
})

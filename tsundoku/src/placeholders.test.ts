import { describe, expect, it } from "vitest"
import { fillPlaceholders, insertPlaceholder } from "./placeholders"

describe("insertPlaceholder", () => {
  it("puts the placeholder where the cursor is", () => {
    expect(insertPlaceholder("D:\\Hydrus\\", "site", 10, 10)).toEqual({ text: "D:\\Hydrus\\{site}", cursor: 16 })
    expect(insertPlaceholder("ab", "uploader", 1, 1)).toEqual({ text: "a{uploader}b", cursor: 11 })
    expect(insertPlaceholder("", "site", 0, 0)).toEqual({ text: "{site}", cursor: 6 })
  })

  it("replaces the selected text", () => {
    expect(insertPlaceholder("D:\\old\\x", "uploader", 3, 6)).toEqual({ text: "D:\\{uploader}\\x", cursor: 13 })
  })
})

describe("fillPlaceholders", () => {
  it("fills the placeholders it knows, in any case", () => {
    const values = { account: "natgeo", post: "DfPQchkG1tg", number: "1" }
    expect(fillPlaceholders("{account} {Post} {NUMBER}", values)).toBe("natgeo DfPQchkG1tg 1")
  })

  it("leaves other braces as they are", () => {
    expect(fillPlaceholders("{post} {title} {x {}", { post: "a" })).toBe("a {title} {x {}")
  })
})

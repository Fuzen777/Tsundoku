import { describe, expect, it } from "vitest"
import { insertPlaceholder, shownFolder } from "./ruleLabels"

describe("insertPlaceholder", () => {
  it("puts the placeholder where the cursor is", () => {
    expect(insertPlaceholder("D:\\Hydrus\\", "site", 10, 10)).toEqual({ folder: "D:\\Hydrus\\{site}", cursor: 16 })
    expect(insertPlaceholder("ab", "uploader", 1, 1)).toEqual({ folder: "a{uploader}b", cursor: 11 })
    expect(insertPlaceholder("", "site", 0, 0)).toEqual({ folder: "{site}", cursor: 6 })
  })

  it("replaces the selected text", () => {
    expect(insertPlaceholder("D:\\old\\x", "uploader", 3, 6)).toEqual({ folder: "D:\\{uploader}\\x", cursor: 13 })
  })
})

describe("shownFolder", () => {
  it("puts a folder that isn't a full path inside the download folder", () => {
    expect(shownFolder("{site}", "C:\\Downloads\\Tsundoku\\")).toBe("C:\\Downloads\\Tsundoku\\{site}")
    expect(shownFolder("\\Saved", "C:\\Downloads\\Tsundoku")).toBe("C:\\Downloads\\Tsundoku\\Saved")
  })

  it("keeps full paths as they are", () => {
    expect(shownFolder("D:\\Hydrus\\{site}", "C:\\Downloads\\Tsundoku")).toBe("D:\\Hydrus\\{site}")
    expect(shownFolder("\\\\nas\\media", "C:\\Downloads\\Tsundoku")).toBe("\\\\nas\\media")
  })
})

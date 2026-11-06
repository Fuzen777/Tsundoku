import { describe, expect, it } from "vitest"
import { shownFolder } from "./ruleLabels"

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

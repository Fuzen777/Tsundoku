import { describe, expect, it } from "vitest"
import type { RuleCondition } from "../../bindings/RuleCondition"
import { capturedGroups, describeConditions, shownFolder } from "./ruleLabels"

const matches = (value: string): RuleCondition => ({ field: "title", operator: "matches", value })

describe("capturedGroups", () => {
  it("counts the groups of the first pattern that has any", () => {
    expect(capturedGroups([matches("episode"), matches(String.raw`^\[(.+?)\] episode (\d+)`)])).toBe(2)
    expect(capturedGroups([{ field: "title", operator: "doesNotMatch", value: "(a)" }])).toBe(0)
    expect(capturedGroups([{ field: "title", operator: "contains", value: "(a)" }])).toBe(0)
  })

  it("stops at nine, and counts a pattern JavaScript can't read as one group", () => {
    expect(capturedGroups([matches("(a)".repeat(12))])).toBe(9)
    expect(capturedGroups([matches("(?P<show>.+)")])).toBe(1)
  })
})

describe("describeConditions", () => {
  it("quotes a pattern, even for the site", () => {
    expect(describeConditions([{ field: "site", operator: "matches", value: "^x$" }], [])).toBe(
      "When the site matches “^x$”",
    )
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

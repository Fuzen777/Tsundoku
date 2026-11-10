import { describe, expect, it } from "vitest"
import type { RuleCondition } from "../../bindings/RuleCondition"
import { capturedGroups, describeConditions, shownFolder } from "./ruleLabels"

const matches = (value: string): RuleCondition => ({ field: "title", operator: "matches", value, caseSensitive: false })

describe("capturedGroups", () => {
  it("counts the groups of the first pattern that has any", () => {
    expect(capturedGroups([matches("episode"), matches(String.raw`^\[(.+?)\] episode (\d+)`)])).toBe(2)
    expect(capturedGroups([{ field: "title", operator: "doesNotMatch", value: "(a)", caseSensitive: false }])).toBe(0)
    expect(capturedGroups([{ field: "title", operator: "contains", value: "(a)", caseSensitive: false }])).toBe(0)
  })

  it("stops at nine, and counts a pattern JavaScript can't read as one group", () => {
    expect(capturedGroups([matches("(a)".repeat(12))])).toBe(9)
    expect(capturedGroups([matches("(?P<show>.+)")])).toBe(1)
  })
})

describe("describeConditions", () => {
  it("quotes a pattern, even for the site", () => {
    expect(
      describeConditions([{ field: "site", operator: "matches", value: "^x$", caseSensitive: false }], "all", []),
    ).toBe("When the site matches “^x$”")
  })

  it("says when a condition matches case", () => {
    const site = { field: "title", operator: "contains", value: "Zoo", caseSensitive: true } as const
    expect(describeConditions([site], "all", [])).toBe("When the title contains “Zoo” (matching case)")
    expect(describeConditions([{ ...site, field: "kind", value: "video" }], "all", [])).toBe(
      "When it's a video",
    )
  })

  it("joins conditions with and, or with or", () => {
    const sites: RuleCondition[] = [
      { field: "site", operator: "is", value: "X", caseSensitive: false },
      { field: "site", operator: "is", value: "Bluesky", caseSensitive: false },
    ]
    expect(describeConditions(sites, "all", [])).toBe("When the site is X and the site is Bluesky")
    expect(describeConditions(sites, "any", [])).toBe("When the site is X or the site is Bluesky")
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

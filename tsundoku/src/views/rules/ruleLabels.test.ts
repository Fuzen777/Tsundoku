import { describe, expect, it } from "vitest"
import type { RuleCondition } from "../../bindings/RuleCondition"
import {
  capturedGroups,
  describeConditions,
  describeManualConditions,
  looksForTags,
  newRule,
  shownFolder,
  withOperator,
} from "./ruleLabels"

const matches = (value: string): RuleCondition => ({
  field: "title",
  operator: "matches",
  value,
  caseSensitive: false,
  join: "all",
})

describe("capturedGroups", () => {
  it("counts the groups of the first pattern that has any", () => {
    expect(capturedGroups([matches("episode"), matches(String.raw`^\[(.+?)\] episode (\d+)`)])).toBe(2)
    expect(
      capturedGroups([{ field: "title", operator: "doesNotMatch", value: "(a)", caseSensitive: false, join: "all" }]),
    ).toBe(0)
    expect(
      capturedGroups([{ field: "title", operator: "contains", value: "(a)", caseSensitive: false, join: "all" }]),
    ).toBe(0)
  })

  it("stops at nine, and counts a pattern JavaScript can't read as one group", () => {
    expect(capturedGroups([matches("(a)".repeat(12))])).toBe(9)
    expect(capturedGroups([matches("(?P<show>.+)")])).toBe(1)
  })
})

describe("describeConditions", () => {
  const condition = (
    field: RuleCondition["field"],
    value: string,
    join: RuleCondition["join"] = "all",
  ): RuleCondition => ({
    field,
    operator: field === "title" ? "contains" : "is",
    value,
    caseSensitive: false,
    join,
  })

  it("quotes a pattern, even for the site", () => {
    expect(describeConditions([{ ...condition("site", "^x$"), operator: "matches" }], [])).toBe(
      "When the site matches “^x$”",
    )
  })

  it("says when a condition matches case", () => {
    const title = { ...condition("title", "Zoo"), caseSensitive: true }
    expect(describeConditions([title], [])).toBe("When the title contains “Zoo” (matching case)")
    expect(describeConditions([{ ...title, field: "kind", operator: "is", value: "video" }], [])).toBe(
      "When it's a video",
    )
  })

  it("joins each condition with its own AND or OR", () => {
    expect(
      describeConditions([condition("site", "X"), condition("site", "Bluesky", "any"), condition("title", "cat")], []),
    ).toBe("When the site is X OR the site is Bluesky AND the title contains “cat”")
  })

  it("names a manual rule's conditions without When, and nothing without any", () => {
    expect(describeManualConditions([matches(String.raw`(RJ\d+)`), condition("site", "DLsite", "any")], [])).toBe(
      String.raw`Conditions: the title matches “(RJ\d+)” OR the site is DLsite`,
    )
    expect(describeManualConditions([], [])).toBeNull()
  })
})

describe("Tags conditions", () => {
  const tags = (operator: RuleCondition["operator"], value: string): RuleCondition => ({
    field: "tags",
    operator,
    value,
    caseSensitive: false,
    join: "all",
  })

  it("says what a Tags condition looks for", () => {
    expect(describeConditions([tags("is", "cats")], [])).toBe("When it has the tag “cats”")
    expect(describeConditions([tags("is", "cats\ndogs")], [])).toBe("When it has one of the tags “cats”, “dogs”")
    expect(describeConditions([tags("isNot", "cats\ndogs")], [])).toBe("When it has none of the tags “cats”, “dogs”")
    expect(describeConditions([tags("isNot", "cats")], [])).toBe("When it doesn't have the tag “cats”")
    expect(describeConditions([tags("doesNotContain", "cat")], [])).toBe("When no tag contains “cat”")
  })

  it("turns a list of tags into text and back when the comparison changes", () => {
    expect(withOperator(tags("is", "cats\ndogs"), "contains").value).toBe("cats")
    expect(withOperator(tags("contains", " cat "), "isNot").value).toBe("cat")
    expect(withOperator(tags("is", "cats\ndogs"), "isNot").value).toBe("cats\ndogs")
  })

  it("offers {tag} only when a condition looks for tags", () => {
    expect(looksForTags([tags("is", "cats")])).toBe(true)
    expect(looksForTags([tags("matches", "^c")])).toBe(true)
    expect(looksForTags([tags("isNot", "cats"), tags("doesNotMatch", "^c")])).toBe(false)
    expect(looksForTags([matches("(a)")])).toBe(false)
  })
})

describe("newRule", () => {
  it("starts a manual rule with a pattern on the title, to fill its placeholders", () => {
    expect(newRule(true)).toMatchObject({ manual: true, conditions: [{ field: "title", operator: "matches" }] })
    expect(newRule()).toMatchObject({ manual: false, conditions: [{ field: "site", operator: "is" }] })
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

import { describe, expect, it } from "vitest"
import type { Preset } from "../../bindings/Preset"
import { canPickPreset, copyOfRule, describeActions, movedRule, newRule } from "./ruleLabels"

const audioMp3 = { id: "audio-mp3", name: "Audio MP3" } as Preset

describe("describeActions", () => {
  it("names the preset and Download Right Away", () => {
    const rule = { ...newRule(), presetId: "audio-mp3", downloadRightAway: true }
    expect(describeActions(rule, [audioMp3])).toEqual(["Starts as Audio MP3", "Downloads right away"])
    expect(describeActions({ ...rule, presetId: null, downloadRightAway: false }, [audioMp3])).toEqual([
      "Waits to be started",
    ])
  })

  it("says nothing for a rule that only picks a folder", () => {
    expect(describeActions({ ...newRule(), folder: "D:\\Music" }, [audioMp3])).toEqual([])
  })

  it("says the default is used when the preset was deleted", () => {
    const rule = { ...newRule(), presetId: "custom-9" }
    expect(describeActions(rule, [audioMp3])).toEqual([
      "Starts with the default preset, since the one it picked was deleted",
    ])
  })
})

describe("canPickPreset", () => {
  it("refuses conditions on the kind or the preset", () => {
    expect(canPickPreset([{ field: "site", operator: "is", value: "SoundCloud", caseSensitive: false }])).toBe(true)
    expect(canPickPreset([{ field: "kind", operator: "is", value: "audio", caseSensitive: false }])).toBe(false)
    expect(canPickPreset([{ field: "preset", operator: "isNot", value: "best-video", caseSensitive: false }])).toBe(
      false,
    )
  })
})

describe("copyOfRule", () => {
  it("keeps everything but the ID, with conditions of its own", () => {
    const rule = { ...newRule(), id: "rule-3", folder: "{site}", presetId: "audio-mp3", enabled: false }
    const copy = copyOfRule(rule)
    expect(copy).toEqual({ ...rule, id: "" })
    expect(copy.conditions[0]).not.toBe(rule.conditions[0])
  })
})

describe("movedRule", () => {
  const rules = ["a", "b", "c", "d"].map((id) => ({ ...newRule(), id }))
  const ids = (list: readonly { id: string }[] | null) => list?.map((rule) => rule.id).join("")

  it("moves a rule before another, up or down, or to the end", () => {
    expect(ids(movedRule(rules, 3, 1))).toBe("adbc")
    expect(ids(movedRule(rules, 1, 3))).toBe("acbd")
    expect(ids(movedRule(rules, 0, 4))).toBe("bcda")
  })

  it("says when the rule stays where it is", () => {
    expect(movedRule(rules, 2, 2)).toBeNull()
    expect(movedRule(rules, 2, 3)).toBeNull()
  })
})

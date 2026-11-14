import { describe, expect, it } from "vitest"
import type { Preset } from "../../bindings/Preset"
import { canPickPreset, copyOfRule, describeActions, movedRule, newRule, ruleLabel } from "./ruleLabels"

const audioMp3 = { id: "audio-mp3", name: "Audio MP3" } as Preset

describe("describeActions", () => {
  it("names the preset and Download Right Away", () => {
    const rule = { ...newRule(), presetId: "audio-mp3", downloadRightAway: true }
    expect(describeActions(rule, [audioMp3])).toEqual(["Starts as Audio MP3", "Downloads right away"])
    expect(describeActions({ ...rule, presetId: null, downloadRightAway: false }, [audioMp3])).toEqual([
      "Waits to be started",
    ])
  })

  it("quotes the file name", () => {
    expect(describeActions({ ...newRule(), fileName: " {post} {number} " }, [audioMp3])).toEqual([
      "Names files “{post} {number}”",
    ])
  })

  it("says when it leaves out placeholders without a value", () => {
    const rule = { ...newRule(), folder: "{site}\\{account}", missingValues: "leaveOut" as const }
    expect(describeActions(rule, [audioMp3])).toEqual(["Leaves out placeholders without a value"])
    expect(describeActions({ ...rule, folder: "", presetId: "audio-mp3" }, [audioMp3])).toEqual(["Starts as Audio MP3"])
  })

  it("says whether and where archives are unpacked", () => {
    expect(describeActions({ ...newRule(), unpack: "inPlace" }, [audioMp3])).toEqual(["Unpacks archives: In Place"])
    expect(describeActions({ ...newRule(), unpack: "archiveFolder" }, [audioMp3])).toEqual([
      "Unpacks archives: In a Folder Named After the Archive",
    ])
    expect(describeActions({ ...newRule(), unpack: "off" }, [audioMp3])).toEqual(["Leaves archives packed"])
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
    expect(
      canPickPreset([{ field: "site", operator: "is", value: "SoundCloud", caseSensitive: false, join: "all" }]),
    ).toBe(true)
    expect(canPickPreset([{ field: "kind", operator: "is", value: "audio", caseSensitive: false, join: "all" }])).toBe(
      false,
    )
    expect(
      canPickPreset([{ field: "preset", operator: "isNot", value: "best-video", caseSensitive: false, join: "all" }]),
    ).toBe(false)
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

describe("ruleLabel", () => {
  it("names a rule by its name, else by its number", () => {
    const rules = [
      { ...newRule(), name: "  " },
      { ...newRule(), name: " Nat Geo " },
    ]
    expect(ruleLabel(rules, 1)).toBe("Rule 1")
    expect(ruleLabel(rules, 2)).toBe("Nat Geo")
    expect(ruleLabel(undefined, 3)).toBe("Rule 3")
  })
})

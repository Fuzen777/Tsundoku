import { describe, expect, it } from "vitest"
import type { Preset } from "../../bindings/Preset"
import { canPickPreset, describeActions, newRule } from "./ruleLabels"

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
    expect(canPickPreset([{ field: "site", operator: "is", value: "SoundCloud" }])).toBe(true)
    expect(canPickPreset([{ field: "kind", operator: "is", value: "audio" }])).toBe(false)
    expect(canPickPreset([{ field: "preset", operator: "isNot", value: "best-video" }])).toBe(false)
  })
})

import { describe, expect, it } from "vitest"
import { tagOf, withTags } from "./tags"

describe("tagOf", () => {
  it("drops the line breaks around a value and keeps its spaces", () => {
    expect(tagOf("\r\n pass word \n")).toBe(" pass word ")
    expect(tagOf("a,b")).toBe("a,b")
  })
})

describe("withTags", () => {
  it("adds new values at the end, in order", () => {
    expect(withTags(["one"], ["two", "three"])).toEqual(["one", "two", "three"])
  })

  it("leaves out blank values and repeats, answering the same list when nothing is new", () => {
    const values = ["one", "two"]
    expect(withTags(values, ["", "   ", "one", "two\r\n"])).toBe(values)
    expect(withTags(values, ["three", "three"])).toEqual(["one", "two", "three"])
  })
})

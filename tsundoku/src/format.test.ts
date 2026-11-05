import { describe, expect, it } from "vitest"
import { formatBytes } from "./format"

describe("formatBytes", () => {
  it("keeps bytes whole", () => {
    expect(formatBytes(0)).toBe("0 B")
    expect(formatBytes(1023)).toBe("1023 B")
  })

  it("uses 1024-based units with one decimal", () => {
    expect(formatBytes(1024)).toBe("1.0 KB")
    expect(formatBytes(23_575_744)).toBe("22.5 MB")
    expect(formatBytes(90_993_957)).toBe("86.8 MB")
    expect(formatBytes(5 * 1024 ** 3)).toBe("5.0 GB")
  })
})

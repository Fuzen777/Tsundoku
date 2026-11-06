import { describe, expect, it } from "vitest"
import { SEND_PAGE_BOOKMARK } from "./sendPage"

describe("the browser bookmark", () => {
  it("sends the page it's on as tsundoku://add?url=…", () => {
    const location = { href: "https://www.youtube.com/watch?v=jNQXAC9IVRw&t=1" }
    const code = SEND_PAGE_BOOKMARK.slice("javascript:".length)
    // Runs the bookmark as a browser would, with a stand-in for the page's location.
    new Function("location", code)(location)
    expect(location.href).toBe(
      "tsundoku://add?url=https%3A%2F%2Fwww.youtube.com%2Fwatch%3Fv%3DjNQXAC9IVRw%26t%3D1",
    )
  })
})

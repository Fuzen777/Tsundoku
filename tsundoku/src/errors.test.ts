import { describe, expect, it } from "vitest"
import { describeHistoryError, describePreviewError, describeToolError, fixFor, isCancelled } from "./errors"
import { formatClockTime } from "./format"
import { strings } from "./strings"

describe("describeToolError", () => {
  it("names the tool and keeps the cause as the detail", () => {
    expect(describeToolError({ kind: "wontRun", detail: "The system cannot execute it." }, "ffmpeg")).toEqual({
      message: "ffmpeg won't run.",
      detail: "The system cannot execute it.",
    })
  })

  it("drops an empty detail", () => {
    expect(describeToolError({ kind: "notThisTool", output: "" }, "deno").detail).toBeUndefined()
  })

  it("treats anything that isn't a core error as unexpected", () => {
    const description = describeToolError(new Error("invoke failed"), "ytDlp")
    expect(description.message).toBe(strings.errors.unexpected)
    expect(description.detail).toBe("Error: invoke failed")
  })
})

describe("describePreviewError", () => {
  it("keeps yt-dlp's error as the detail", () => {
    const description = describePreviewError({ kind: "unavailable", detail: "[youtube] x: This video is unavailable" })
    expect(description).toEqual({
      message: strings.preview.errors.unavailable,
      detail: "[youtube] x: This video is unavailable",
    })
  })

  it("describes yt-dlp or gallery-dl failing to run as that tool's error", () => {
    expect(describePreviewError({ kind: "tool", tool: "ytDlp", error: { kind: "blocked" } }).message).toBe(
      strings.tools.errors.blocked("yt-dlp"),
    )
    expect(describePreviewError({ kind: "tool", tool: "galleryDl", error: { kind: "blocked" } }).message).toBe(
      strings.tools.errors.blocked("gallery-dl"),
    )
  })
})

describe("pages of several items", () => {
  it("points a listing in Pending to the add bar, and names a site that can't be listed, without naming a tool", () => {
    const listing = describePreviewError({ kind: "playlist" }).message
    expect(listing).toContain("add bar")
    const refused = describePreviewError({ kind: "notListable", site: "X" }).message
    expect(refused).toBe(
      "This X page holds several items, which Tsundoku can't list yet. Paste a link to a single one instead.",
    )
    expect(refused).not.toMatch(/yt-dlp|gallery-dl/)
    // Either may be a playlist of songs or a folder of files, so neither calls what it holds videos or posts.
    expect([listing, refused].join(" ")).not.toMatch(/videos|posts/)
  })
})

describe("describeHistoryError", () => {
  it("shows a missing file's path", () => {
    expect(describeHistoryError({ kind: "fileMissing", path: String.raw`D:\Videos\a.mkv` })).toEqual({
      message: strings.history.errors.fileMissing,
      detail: String.raw`D:\Videos\a.mkv`,
    })
  })
})

describe("isCancelled", () => {
  it("only matches the core's cancellation", () => {
    expect(isCancelled({ kind: "cancelled" })).toBe(true)
    expect(isCancelled({ kind: "busy" })).toBe(false)
    expect(isCancelled(new Error("cancelled"))).toBe(false)
  })
})

describe("the sign-in hint", () => {
  it("says what to do about a missing sign-in", () => {
    const signIn = (site: string | null, cookies: string) =>
      describePreviewError({ kind: "signInRequired", detail: "x", site, cookies }).message
    expect(signIn("Instagram", "notSignedIn")).toBe(
      "Instagram needs you signed in. Sign in to Instagram in your browser, then try again.",
    )
    expect(signIn("X", "signedIn")).toContain("didn't accept your browser's sign-in")
    expect(signIn(null, "off")).toContain("Turn on cookies")
    expect(signIn(null, "unknown")).toBe("This needs a signed-in account or an age check.")
  })
})

describe("fixFor", () => {
  it("offers updating yt-dlp when a site's page couldn't be read", () => {
    expect(fixFor({ kind: "siteError", tool: "ytDlp", detail: "[youtube] x: Failed to extract" })).toBe("updateYtDlp")
    expect(fixFor({ kind: "siteError", tool: "galleryDl", detail: "KeyError: 'data'" })).toBe("updateGalleryDl")
  })

  it("offers setting up image downloads for a post of images", () => {
    expect(fixFor({ kind: "imagesNotSetUp" })).toBe("setUpImages")
  })

  it("offers the cookies setting when the video needs a signed-in account, for a download too", () => {
    expect(fixFor({ kind: "ytDlp", error: { kind: "signInRequired", detail: "Sign in to confirm your age" } })).toBe(
      "cookies",
    )
  })

  it("offers trying again once signed in when cookies are on", () => {
    const signIn = { kind: "signInRequired", detail: "login page", site: "Instagram" }
    expect(fixFor({ ...signIn, cookies: "notSignedIn" })).toBe("signIn")
    expect(fixFor({ ...signIn, cookies: "signedIn" })).toBe("signIn")
    expect(fixFor({ ...signIn, cookies: "off" })).toBe("cookies")
  })

  it("offers nothing for other failures", () => {
    expect(fixFor({ kind: "unavailable", detail: "gone" })).toBeNull()
    expect(fixFor({ kind: "noFile" })).toBeNull()
    expect(fixFor(new Error("x"))).toBeNull()
  })
})

describe("file host errors", () => {
  it("names the host", () => {
    expect(describePreviewError({ kind: "browserCheck", site: "MediaFire" }).message).toBe(
      strings.preview.errors.browserCheck("MediaFire"),
    )
    expect(describePreviewError({ kind: "linkIncomplete", site: "MEGA" }).message).toBe(
      strings.preview.errors.linkIncomplete("MEGA"),
    )
    expect(describePreviewError({ kind: "linkNotRecognized", site: "MediaFire" }).message).toBe(
      strings.preview.errors.linkNotRecognized("MediaFire"),
    )
    expect(
      describePreviewError({ kind: "noSupportedMirror", site: "MultiUp", mirrors: ["clicknupload.click"] }).message,
    ).toBe(strings.preview.errors.noSupportedMirror("MultiUp", ["clicknupload.click"]))
    expect(describePreviewError({ kind: "shortLinkPage", site: "Bitly" })).toEqual({
      message:
        "This Bitly link shows a page before leading anywhere, so Tsundoku can't follow it. Open it in your browser, then copy the link it ends on and add that instead.",
    })
    expect(
      describePreviewError({ kind: "shortLinkToUnsupportedSite", site: "Bitly", target: "example.com" }),
    ).toEqual({
      message: "This Bitly link leads to example.com, which Tsundoku can't download from.",
    })
    expect(describePreviewError({ kind: "subscribersOnly", site: "1fichier" })).toEqual({
      message: strings.preview.errors.subscribersOnly("1fichier"),
    })
    expect(describePreviewError({ kind: "waitBetweenDownloads", site: "1fichier", seconds: 600 }).message).toContain(
      "1fichier makes free downloads wait",
    )
    expect(describePreviewError({ kind: "limitReached", site: "MEGA", detail: "MEGA answered -17" })).toEqual({
      message: strings.preview.errors.limitReached("MEGA"),
      detail: "MEGA answered -17",
    })
  })
})

describe("a site that asks to slow down", () => {
  it("says which site and from when to try again, without naming the tool", () => {
    const until = Date.UTC(2026, 10, 6, 14, 32)
    const message = describePreviewError({ kind: "tooManyRequests", detail: "HTTP Error 429", site: "Instagram", until })
      .message
    expect(message).toBe(strings.preview.errors.tooManyRequests("Instagram", formatClockTime(until)))
    expect(message).toMatch(/^Instagram asked to slow down\. Try again after /)
    expect(describePreviewError({ kind: "tooManyRequests", detail: "", site: null, until: null })).toEqual({
      message: "The site asked to slow down. Try again in a few minutes.",
      detail: undefined,
    })
  })
})

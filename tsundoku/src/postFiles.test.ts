import { describe, expect, it } from "vitest"
import type { PostFile } from "./bindings/PostFile"
import type { VideoInfo } from "./bindings/VideoInfo"
import { defaultPick, pickedOf, pickSummary, withPicked } from "./postFiles"

const files: PostFile[] = [1, 2, 3, 4].map((number) => ({
  number,
  video: number > 2,
  thumbnailUrl: null,
  picked: true,
}))

const post: VideoInfo = {
  site: "instagram",
  siteName: "instagram.com",
  videoId: "Abc",
  url: "https://www.instagram.com/p/Abc/",
  title: "A carousel",
  uploader: "someone",
  durationSeconds: null,
  uploadDate: null,
  thumbnailUrl: null,
  bestHeight: null,
  post: { images: 2, videos: 2, others: 0 },
  files: null,
  postFiles: files,
}

describe("defaultPick", () => {
  it("ticks every file of a post never downloaded, or downloaded whole", () => {
    expect([...defaultPick(files, null)]).toEqual([1, 2, 3, 4])
    expect([...defaultPick(files, [])]).toEqual([1, 2, 3, 4])
  })

  it("ticks only the files left out last time", () => {
    expect([...defaultPick(files, [2, 4])]).toEqual([2, 4])
  })
})

describe("withPicked and pickedOf", () => {
  it("unticks the files not picked, and reads them back", () => {
    const picked = withPicked(post, new Set([1, 3]))
    expect(picked.postFiles?.map((file) => file.picked)).toEqual([true, false, true, false])
    expect([...pickedOf(picked.postFiles ?? [])]).toEqual([1, 3])
    expect(post.postFiles?.every((file) => file.picked)).toBe(true)
  })
})

describe("pickSummary", () => {
  it("says how many files a post's download takes when it leaves some out", () => {
    expect(pickSummary(withPicked(post, new Set([1, 3, 4])))).toBe("3 of 4 files")
  })

  it("says nothing when it takes them all, or isn't a post", () => {
    expect(pickSummary(post)).toBeNull()
    expect(pickSummary({ ...post, post: null, postFiles: undefined })).toBeNull()
  })
})

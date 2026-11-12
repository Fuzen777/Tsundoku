import { describe, expect, it } from "vitest"
import type { PostFile } from "./bindings/PostFile"
import type { VideoInfo } from "./bindings/VideoInfo"
import { defaultPick, listsPostFiles, pickedOf, pickSummary, postLeftOutMessage, postListFiles, withPicked } from "./postFiles"
import { strings } from "./strings"

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

describe("a post of files", () => {
  const named: PostFile[] = [
    { number: 1, video: false, thumbnailUrl: null, picked: true, name: "Build.zip", size: 734_003_200 },
    { number: 2, video: false, thumbnailUrl: null, picked: true, name: "Notes/Read Me.txt" },
    { number: 3, video: false, thumbnailUrl: null, picked: true },
  ]

  it("shows as a list when some files aren't images or videos", () => {
    expect(listsPostFiles(post)).toBe(false)
    expect(listsPostFiles({ ...post, post: { images: 1, videos: 0, others: 2 } })).toBe(true)
    expect(listsPostFiles({ ...post, post: null })).toBe(false)
  })

  it("lists each file by its name, else its number, with its size when known, never in folders", () => {
    expect(postListFiles(named)).toEqual([
      { path: "Build.zip", size: 734_003_200, number: 1 },
      { path: "Notes_Read Me.txt", size: null, number: 2 },
      { path: "File 3", size: null, number: 3 },
    ])
  })

  it("says the files left out are the ones ticked only while they are", () => {
    expect(postLeftOutMessage([2, 4], new Set([2, 4]))).toBe(strings.postFiles.leftOut(2))
    expect(postLeftOutMessage([2, 4], new Set([1, 2, 3, 4]))).toBe(strings.postFiles.leftOutCount(2))
    expect(postLeftOutMessage([2], new Set([1]))).toBe("1 file wasn't picked last time.")
  })
})

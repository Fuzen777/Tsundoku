import type { AudioFormat } from "../../bindings/AudioFormat"
import type { Preset } from "../../bindings/Preset"
import type { VideoContainer } from "../../bindings/VideoContainer"
import { strings } from "../../strings"

/** The heights offered for video presets; `null` is Best. */
export const VIDEO_HEIGHTS: readonly (number | null)[] = [null, 2160, 1440, 1080, 720, 480, 360]
export const VIDEO_CONTAINERS: readonly VideoContainer[] = ["mkv", "mp4", "original"]
export const AUDIO_FORMATS: readonly AudioFormat[] = ["original", "mp3", "m4a", "opus", "flac"]

export function heightLabel(height: number | null): string {
  return height === null ? strings.presets.fields.best : strings.presets.fields.upTo(height)
}

/** One line about what a preset makes: "Video · Up to 1080p · MP4". */
export function presetSummary(preset: Preset): string {
  const { output } = preset
  const fields = strings.presets.fields
  return output.kind === "video"
    ? strings.presets.summary.video(heightLabel(output.maxHeight), fields.containers[output.container])
    : strings.presets.summary.audio(fields.audioFormats[output.format])
}

/** yt-dlp's arguments as they'd be typed, quoting any with spaces. */
export function argumentsLine(args: readonly string[]): string {
  return args.map((arg) => (arg === "" || /\s/.test(arg) ? `"${arg}"` : arg)).join(" ")
}

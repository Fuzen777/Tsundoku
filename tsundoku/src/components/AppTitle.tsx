import { strings } from "../strings"

/** The app's name, top left, with 積ん読 beside it. */
export function AppTitle() {
  return (
    <div className="flex items-baseline gap-2 px-5 py-4">
      <span className="text-base font-semibold">{strings.appName}</span>
      <span className="text-xs text-fg-muted" lang="ja">
        {strings.appNameJapanese}
      </span>
    </div>
  )
}

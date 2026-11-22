import { useState } from "react"
import type { WatchedSource } from "../bindings/WatchedSource"
import { customDraftOf, pickedDraft, type PictureDraft } from "../pictureDraft"
import { useWatchedPictureChoices } from "../queries"
import { strings } from "../strings"
import { PictureCropper } from "./PictureCropper"
import { SegmentedControl } from "./SegmentedControl"
import { classes } from "./styles"
import { sitePictureUrl } from "../api"

const text = strings.watched.pictures

interface SourcePictureFieldProps {
  source: WatchedSource
  draft: PictureDraft
  onChange: (draft: PictureDraft) => void
  disabled: boolean
}

/**
 * Edit's Picture: Automatic (the listing's own) or Custom, which shows the picture in a frame of the source's shape to
 * pick the part shown, beside the source's pictures to pick another from.
 */
export function SourcePictureField({ source, draft, onChange, disabled }: SourcePictureFieldProps) {
  return (
    <div className="flex flex-col gap-1.5">
      <span className="font-medium">{strings.watched.picture}</span>
      <SegmentedControl
        label={strings.watched.picture}
        options={[
          { value: "automatic", label: text.automatic },
          { value: "custom", label: text.custom },
        ]}
        value={draft.kind}
        disabled={disabled}
        onChange={(kind) => onChange(kind === "automatic" ? { kind } : customDraftOf(source))}
      />
      <p className="text-xs text-fg-muted">
        {draft.kind === "automatic" ? text.automaticHint[source.kind] : text.customHint}
      </p>
      {draft.kind === "custom" && (
        <div className="flex flex-wrap items-start gap-4 pt-1">
          {draft.url ? (
            <PictureCropper
              key={draft.url}
              kind={source.kind}
              url={draft.url}
              crop={draft.crop}
              disabled={disabled}
              onChange={(crop) => onChange({ ...draft, crop })}
            />
          ) : (
            <p className="flex h-48 w-72 shrink-0 items-center justify-center rounded-md bg-input p-4 text-center text-xs text-fg-secondary">
              {text.pickOne}
            </p>
          )}
          <PictureChoices
            sourceId={source.id}
            picked={draft.stored ? null : draft.url}
            disabled={disabled}
            onPick={(url) => onChange(pickedDraft(url))}
          />
        </div>
      )}
    </div>
  )
}

/** The source's pictures, the listing's first, as small squares to pick from; those that don't load (a site's link
 * that expired) are left out. */
function PictureChoices({
  sourceId,
  picked,
  disabled,
  onPick,
}: {
  sourceId: number
  picked: string | null
  disabled: boolean
  onPick: (url: string) => void
}) {
  const { data: choices, isPending } = useWatchedPictureChoices(sourceId, true)
  const [broken, setBroken] = useState<ReadonlySet<string>>(new Set())
  const shown = (choices ?? []).filter((url) => !broken.has(url))
  return (
    <div className="flex min-w-56 flex-1 flex-col gap-1.5">
      <p className="text-xs text-fg-secondary tabular-nums">
        {isPending ? text.loading : shown.length === 0 ? text.none : text.choices(shown.length)}
      </p>
      {shown.length > 0 && (
        <ul className="grid max-h-60 grid-cols-[repeat(auto-fill,minmax(3.5rem,1fr))] gap-1.5 overflow-y-auto p-0.5">
          {shown.map((url, index) => (
            <li key={url}>
              <button
                type="button"
                aria-label={text.choice(index + 1)}
                aria-pressed={url === picked}
                disabled={disabled}
                onClick={() => onPick(url)}
                className={classes(
                  "block aspect-square w-full overflow-hidden rounded-md bg-input transition-opacity hover:opacity-80 focus-visible:outline-2 focus-visible:outline-accent disabled:opacity-40",
                  url === picked && "ring-2 ring-accent",
                )}
              >
                <img
                  src={sitePictureUrl(url)}
                  alt=""
                  loading="lazy"
                  referrerPolicy="no-referrer"
                  draggable={false}
                  className="size-full object-cover"
                  onError={() => setBroken((current) => new Set(current).add(url))}
                />
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}

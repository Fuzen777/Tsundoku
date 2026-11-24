import { useEffect, useLayoutEffect, useRef, useState, type Ref } from "react"
import {
  closeWindow,
  minimizeWindow,
  onMaximizeButtonState,
  onWindowFocusChanged,
  onWindowMaximizedChanged,
  placeMaximizeButton,
  toggleMaximizeWindow,
} from "../api"
import type { MaximizeButtonState } from "../bindings/MaximizeButtonState"
import { strings } from "../strings"
import { classes } from "./styles"
import { Tooltip } from "./Tooltip"

/** Windows' own glyphs for the caption buttons, in its icon font (`font-caption`). */
const GLYPHS = {
  minimize: "",
  maximize: "",
  restore: "",
  close: "",
}

/**
 * Minimize, Maximize (Restore Down while maximized) and Close, at the window's top right as Windows draws them: the
 * window has no frame of its own (`title_bar.rs`). They take the title bar's whole height, flush with the window's
 * right edge, their glyphs muted while the window isn't in front. A window of the app's lies over Maximize so that
 * Windows shows Snap Layouts there; it does the click and says where the pointer is, which the button shows.
 */
export function WindowControls() {
  const [maximized, setMaximized] = useState(false)
  const [focused, setFocused] = useState(true)
  const [maximizeState, setMaximizeState] = useState<MaximizeButtonState>("rest")
  const maximizeButton = useRef<HTMLButtonElement>(null)

  useEffect(() => {
    const listeners = [
      onWindowMaximizedChanged(setMaximized),
      onWindowFocusChanged(setFocused),
      onMaximizeButtonState(setMaximizeState),
    ]
    return () => listeners.forEach((listener) => void listener.then((unlisten) => unlisten()))
  }, [])

  // The window over Maximize is placed from the window's top right corner, so it follows the window's size by itself;
  // it only needs placing again when the button itself changes.
  useLayoutEffect(() => {
    const button = maximizeButton.current
    if (!button) return
    const place = () => {
      const box = button.getBoundingClientRect()
      void placeMaximizeButton({ right: window.innerWidth - box.right, top: box.top, width: box.width, height: box.height })
    }
    place()
    const observer = new ResizeObserver(place)
    observer.observe(button)
    return () => observer.disconnect()
  }, [])

  return (
    <div className={classes("flex shrink-0 self-stretch", focused ? "text-fg" : "text-fg-muted")}>
      <CaptionButton label={strings.window.minimize} glyph={GLYPHS.minimize} onClick={() => void minimizeWindow()} />
      <CaptionButton
        ref={maximizeButton}
        label={maximized ? strings.window.restore : strings.window.maximize}
        glyph={maximized ? GLYPHS.restore : GLYPHS.maximize}
        pointer={maximizeState}
        onClick={() => void toggleMaximizeWindow()}
      />
      <CaptionButton label={strings.window.close} glyph={GLYPHS.close} close onClick={() => void closeWindow()} />
    </div>
  )
}

interface CaptionButtonProps {
  label: string
  glyph: string
  onClick: () => void
  /** Close: red under the pointer, as Windows' own. */
  close?: boolean
  /** Where the pointer is, when the app says so rather than the WebView (Maximize, under its window). */
  pointer?: MaximizeButtonState
  ref?: Ref<HTMLButtonElement>
}

/** A caption button, 46 px wide as Windows' own, out of the keyboard's way as Windows' are (its menu, Alt+Space,
 * holds the same actions). */
function CaptionButton({ label, glyph, onClick, close = false, pointer = "rest", ref }: CaptionButtonProps) {
  return (
    <Tooltip label={label}>
      <button
        ref={ref}
        type="button"
        tabIndex={-1}
        aria-label={label}
        data-pointer={pointer}
        onClick={onClick}
        className={classes(
          "flex w-11.5 items-center justify-center font-caption text-[10px] transition-colors",
          close
            ? "hover:bg-close hover:text-on-close active:bg-close-pressed"
            : "hover:bg-input hover:text-fg active:bg-input-hover data-[pointer=hovered]:bg-input data-[pointer=hovered]:text-fg data-[pointer=pressed]:bg-input-hover data-[pointer=pressed]:text-fg",
        )}
      >
        <span aria-hidden>{glyph}</span>
      </button>
    </Tooltip>
  )
}

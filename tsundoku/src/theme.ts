import type { Accent } from "./bindings/Accent"
import type { AppearanceSettings } from "./bindings/AppearanceSettings"
import type { Theme } from "./bindings/Theme"

/**
 * Makes the UI light, dark, or follow Windows (Settings → Appearance): index.css sets `color-scheme` from
 * `data-theme`, so every light-dark() token, form control and scrollbar follows. The window's title bar and Mica follow
 * through the core, which sets the window's theme when the choice is saved and at startup.
 */
export function applyTheme(theme: Theme) {
  document.documentElement.dataset.theme = theme
}

/** Colors primary buttons, selections and links with the accent picked in Settings → Appearance (`data-accent`). */
export function applyAccent(accent: Accent) {
  document.documentElement.dataset.accent = accent
}

/** The theme and the accent, as Settings → Appearance has them. */
export function applyAppearance({ theme, accent }: AppearanceSettings) {
  applyTheme(theme)
  applyAccent(accent)
}

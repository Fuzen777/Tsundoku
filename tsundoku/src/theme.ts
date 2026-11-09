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

/**
 * Colors primary buttons, selections and links with the accent picked in Settings → Appearance (`data-accent`).
 * The color picked for Custom is always set (`--accent-custom`), so its swatch shows it whichever accent is on.
 */
export function applyAccent(accent: Accent, customAccent: string) {
  const root = document.documentElement
  root.dataset.accent = accent
  root.style.setProperty("--accent-custom", customAccent)
}

/** The theme and the accent, as Settings → Appearance has them. */
export function applyAppearance({ theme, accent, customAccent }: AppearanceSettings) {
  applyTheme(theme)
  applyAccent(accent, customAccent)
}

import type { Theme } from "./bindings/Theme"

/**
 * Makes the UI light, dark, or follow Windows (Settings → Appearance): index.css sets `color-scheme` from
 * `data-theme`, so every light-dark() token, form control and scrollbar follows. The window's title bar and Mica follow
 * through the core, which sets the window's theme when the choice is saved and at startup.
 */
export function applyTheme(theme: Theme) {
  document.documentElement.dataset.theme = theme
}

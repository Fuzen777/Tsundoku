import { useEffect, useEffectEvent } from "react"
import { mainNavItems, settingsNavItem, type ViewId } from "./navigation"
import { strings } from "./strings"

/**
 * The window's keyboard shortcuts: Ctrl+V anywhere outside a text field adds what was copied, Ctrl+L goes to the add
 * bar, Ctrl+1 to Ctrl+7 open the sidebar's views in its order, and Delete removes the links ticked in Pending. Each
 * needs Ctrl alone or no modifier, so none is one of the clipboard shortcuts Settings offers (Ctrl+Alt+V and the
 * like), which Windows takes before the window sees them anyway.
 */

/** The views Ctrl+1, Ctrl+2… open: the sidebar's, in its order, Settings last. */
export const SHORTCUT_VIEWS: readonly ViewId[] = [...mainNavItems.map((item) => item.id), settingsNavItem.id]

/** The keys that open a view ("Ctrl+1"), for its hover text. */
export function viewKeys(view: ViewId): string {
  return strings.shortcuts.view(SHORTCUT_VIEWS.indexOf(view) + 1)
}

export type Shortcut = { kind: "focusAddBar" } | { kind: "view"; view: ViewId }

/** What a key press holds, as a KeyboardEvent has it. */
export interface KeyPress {
  key: string
  code: string
  ctrlKey: boolean
  altKey: boolean
  shiftKey: boolean
  metaKey: boolean
}

/**
 * The shortcut a key press is, if any: Ctrl alone with L, or with a digit for a view. Digits go by the key's place,
 * since on an AZERTY keyboard Ctrl+1 is the key that types "&"; L goes by the letter typed.
 */
export function shortcutFor(press: KeyPress): Shortcut | null {
  if (!press.ctrlKey || press.altKey || press.shiftKey || press.metaKey) return null
  if (press.key.toLowerCase() === "l") return { kind: "focusAddBar" }
  const digit = /^(?:Digit|Numpad)([1-9])$/.exec(press.code)
  const view = digit ? SHORTCUT_VIEWS[Number(digit[1]) - 1] : undefined
  return view ? { kind: "view", view } : null
}

/** Whether a key press is Delete on its own. */
export function isDeleteKey(press: KeyPress): boolean {
  return press.key === "Delete" && !press.ctrlKey && !press.altKey && !press.shiftKey && !press.metaKey
}

/** Inputs that take no typing, where Ctrl+V and Delete stay the window's. */
const NON_TEXT_INPUTS = new Set(["checkbox", "radio", "button", "submit", "reset", "range", "color", "file", "image"])

/**
 * Whether the focused element takes typing (a text field, a select, anything editable), where Ctrl+V and Delete keep
 * their usual meaning. Looks only at the fields it needs, so tests can pass a stand-in.
 */
export function isTextEntry(focused: unknown): boolean {
  if (typeof focused !== "object" || focused === null) return false
  const { tagName, type, isContentEditable } = focused as { tagName?: string; type?: string; isContentEditable?: boolean }
  if (isContentEditable) return true
  switch (tagName) {
    case "TEXTAREA":
    case "SELECT":
      return true
    case "INPUT":
      return !NON_TEXT_INPUTS.has((type ?? "text").toLowerCase())
    default:
      return false
  }
}

/**
 * Calls `onPaste` with the text pasted outside a text field (Ctrl+V, or Shift+Insert), for the add bar to take as if
 * it were pasted there and submitted. Chromium fires `paste` at the focused element, or the page, even when nothing
 * there is editable, and the event carries the text without asking for clipboard access.
 */
export function useWindowPaste(onPaste: (text: string) => void, enabled: boolean) {
  const pasted = useEffectEvent((event: ClipboardEvent) => {
    if (event.defaultPrevented || isTextEntry(document.activeElement)) return
    const text = event.clipboardData?.getData("text/plain").trim()
    if (!text) return
    event.preventDefault()
    onPaste(text)
  })
  useEffect(() => {
    if (!enabled) return
    document.addEventListener("paste", pasted)
    return () => document.removeEventListener("paste", pasted)
  }, [enabled])
}

interface ShortcutActions {
  focusAddBar: () => void
  navigate: (view: ViewId) => void
}

/** Ctrl+L and Ctrl+1 to Ctrl+7, wherever the focus is. The WebView does nothing of its own with them. */
export function useAppShortcuts(actions: ShortcutActions, enabled: boolean) {
  const pressed = useEffectEvent((event: KeyboardEvent) => {
    if (event.defaultPrevented) return
    const shortcut = shortcutFor(event)
    if (!shortcut) return
    event.preventDefault()
    if (shortcut.kind === "focusAddBar") actions.focusAddBar()
    else actions.navigate(shortcut.view)
  })
  useEffect(() => {
    if (!enabled) return
    window.addEventListener("keydown", pressed)
    return () => window.removeEventListener("keydown", pressed)
  }, [enabled])
}

/** Delete outside a text field, while `onDelete` is given. */
export function useDeleteKey(onDelete: (() => void) | null) {
  const enabled = onDelete !== null
  const pressed = useEffectEvent((event: KeyboardEvent) => {
    if (event.defaultPrevented || !isDeleteKey(event) || isTextEntry(document.activeElement)) return
    event.preventDefault()
    onDelete?.()
  })
  useEffect(() => {
    if (!enabled) return
    window.addEventListener("keydown", pressed)
    return () => window.removeEventListener("keydown", pressed)
  }, [enabled])
}

/**
 * Whether a right-click keeps the WebView's own menu: in a text field, where it offers Cut, Copy and Paste, or over
 * selected text, where it offers Copy. Anywhere else its Back, Reload, Save As and Print would act on the app's page
 * as if it were a website, so the click opens nothing.
 */
export function keepsPageMenu(clicked: unknown, selectedText: string): boolean {
  if (selectedText.trim() !== "") return true
  return isTextEntry(clicked) && (clicked as { tagName?: string }).tagName !== "SELECT"
}

/** Turns off the WebView's right-click menu outside text fields and selected text, for the whole window. */
export function limitPageMenu() {
  document.addEventListener("contextmenu", (event) => {
    if (!keepsPageMenu(event.target, window.getSelection()?.toString() ?? "")) event.preventDefault()
  })
}

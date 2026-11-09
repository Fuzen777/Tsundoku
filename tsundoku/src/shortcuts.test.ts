import { describe, expect, it } from "vitest"
import { isDeleteKey, isTextEntry, keepsPageMenu, shortcutFor, SHORTCUT_VIEWS, type KeyPress, viewKeys } from "./shortcuts"

function press(key: string, code: string, modifiers: Partial<KeyPress> = {}): KeyPress {
  return { key, code, ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, ...modifiers }
}

describe("the window's shortcuts", () => {
  it("opens the sidebar's views with Ctrl and a digit, in its order, Settings last", () => {
    expect(SHORTCUT_VIEWS).toEqual(["queue", "pending", "history", "presets", "rules", "settings"])
    expect(shortcutFor(press("1", "Digit1", { ctrlKey: true }))).toEqual({ kind: "view", view: "queue" })
    expect(shortcutFor(press("6", "Numpad6", { ctrlKey: true }))).toEqual({ kind: "view", view: "settings" })
    expect(shortcutFor(press("7", "Digit7", { ctrlKey: true }))).toBeNull()
    expect(viewKeys("pending")).toBe("Ctrl+2")
  })

  it("goes by a digit key's place, so an AZERTY keyboard's Ctrl+1 works", () => {
    expect(shortcutFor(press("&", "Digit1", { ctrlKey: true }))).toEqual({ kind: "view", view: "queue" })
    expect(shortcutFor(press("é", "Digit2", { ctrlKey: true }))).toEqual({ kind: "view", view: "pending" })
  })

  it("goes to the add bar with Ctrl+L, by the letter typed", () => {
    expect(shortcutFor(press("l", "KeyL", { ctrlKey: true }))).toEqual({ kind: "focusAddBar" })
    expect(shortcutFor(press("L", "KeyL", { ctrlKey: true }))).toEqual({ kind: "focusAddBar" })
    expect(shortcutFor(press("l", "KeyL"))).toBeNull()
  })

  it("leaves any other modifier alone, such as the clipboard shortcuts' Ctrl+Alt and AltGr", () => {
    expect(shortcutFor(press("l", "KeyL", { ctrlKey: true, altKey: true }))).toBeNull()
    expect(shortcutFor(press("1", "Digit1", { ctrlKey: true, shiftKey: true }))).toBeNull()
    expect(shortcutFor(press("1", "Digit1", { ctrlKey: true, metaKey: true }))).toBeNull()
  })

  it("takes Delete only on its own", () => {
    expect(isDeleteKey(press("Delete", "Delete"))).toBe(true)
    expect(isDeleteKey(press("Delete", "Delete", { shiftKey: true }))).toBe(false)
    expect(isDeleteKey(press("Backspace", "Backspace"))).toBe(false)
  })
})

describe("text entry", () => {
  it("is a text field, a select or anything editable", () => {
    expect(isTextEntry({ tagName: "INPUT", type: "text" })).toBe(true)
    expect(isTextEntry({ tagName: "INPUT", type: "search" })).toBe(true)
    expect(isTextEntry({ tagName: "TEXTAREA" })).toBe(true)
    expect(isTextEntry({ tagName: "SELECT" })).toBe(true)
    expect(isTextEntry({ tagName: "DIV", isContentEditable: true })).toBe(true)
  })

  it("isn't a checkbox, a button or the page", () => {
    expect(isTextEntry({ tagName: "INPUT", type: "checkbox" })).toBe(false)
    expect(isTextEntry({ tagName: "BUTTON", type: "button" })).toBe(false)
    expect(isTextEntry({ tagName: "BODY", isContentEditable: false })).toBe(false)
    expect(isTextEntry(null)).toBe(false)
  })

  it("keeps the right-click menu only in text fields and over selected text", () => {
    expect(keepsPageMenu({ tagName: "INPUT", type: "text" }, "")).toBe(true)
    expect(keepsPageMenu({ tagName: "TEXTAREA" }, "")).toBe(true)
    expect(keepsPageMenu({ tagName: "DIV" }, "Some title")).toBe(true)
    expect(keepsPageMenu({ tagName: "DIV" }, "")).toBe(false)
    expect(keepsPageMenu({ tagName: "DIV" }, "  ")).toBe(false)
    expect(keepsPageMenu({ tagName: "SELECT" }, "")).toBe(false)
    expect(keepsPageMenu({ tagName: "BUTTON", type: "button" }, "")).toBe(false)
    expect(keepsPageMenu(null, "")).toBe(false)
  })
})

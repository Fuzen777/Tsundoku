/**
 * Every piece of UI text, in English, kept here so translations can come later. Never write UI text in a component.
 * Labels are Title Case; sentences (descriptions, messages) are sentence case. See DESIGN.md → Labels and Casing.
 */
export const strings = {
  appName: "Tsundoku",
  appNameJapanese: "積ん読",
  nav: {
    queue: "Queue",
    history: "History",
    presets: "Presets",
    settings: "Settings",
  },
  addBar: {
    placeholder: "Paste a Video Link",
    add: "Add",
  },
  queue: {
    emptyTitle: "Nothing in the Queue",
    emptyBody: "Paste a video link above to download it.",
  },
  history: {
    emptyTitle: "No Downloads Yet",
    emptyBody: "Everything you download is listed here, so you never get the same video twice.",
  },
  settings: {
    about: "About",
    version: (version: string) => `Version ${version}`,
  },
} as const

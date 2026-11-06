/**
 * The address of the browser bookmark that sends the page it's on to Tsundoku: it opens `tsundoku://add?url=…`, which
 * Windows hands to Tsundoku (the core's links_in_sent_link reads it). Browsers run a `javascript:` bookmark on the
 * page, whatever the page's own rules, and opening another app's link leaves the page where it is.
 */
export const SEND_PAGE_BOOKMARK =
  "javascript:void(location.href='tsundoku://add?url='+encodeURIComponent(location.href))"

/**
 * The userscript that puts a Send to Tsundoku button on Instagram and X posts and YouTube videos, which opens the
 * same `tsundoku://add?url=…` link as the bookmark. Settings copies it for Tampermonkey or Violentmonkey; the file
 * is `tsundoku/userscripts/tsundoku-send-buttons.user.js`, whose `@updateURL` lets the add-on update it from GitHub.
 */
export { default as SEND_BUTTONS_USERSCRIPT } from "../userscripts/tsundoku-send-buttons.user.js?raw"

// ==UserScript==
// @name         Tsundoku Send Buttons
// @namespace    https://github.com/Fuzen777/Tsundoku
// @version      1.0.0
// @description  Adds a Send to Tsundoku button to Instagram and X posts, YouTube videos and Shorts, and YouTube's thumbnails
// @match        https://www.instagram.com/*
// @match        https://x.com/*
// @match        https://twitter.com/*
// @match        https://www.youtube.com/*
// @run-at       document-idle
// @grant        none
// @noframes
// @updateURL    https://raw.githubusercontent.com/Fuzen777/Tsundoku/main/tsundoku/userscripts/tsundoku-send-buttons.user.js
// @downloadURL  https://raw.githubusercontent.com/Fuzen777/Tsundoku/main/tsundoku/userscripts/tsundoku-send-buttons.user.js
// ==/UserScript==

// One click sends a post or video to Tsundoku through the link Settings → Adding Links → Send from Your Browser uses,
// tsundoku://add?url=…, which Windows hands to Tsundoku: the link joins Pending without opening the post or copying
// anything. Each site has a section below. They find posts by roles, labels, link shapes and data-testid rather than
// class names, which the sites change often, and the page is scanned again whenever it changes, since all three load
// posts as you scroll and move between pages without reloading. Nothing is inserted twice.

(function () {
  "use strict";

  const PREFIX = "tsundoku-send";
  const LABEL = "Send to Tsundoku";
  const SENT_LABEL = "Sent to Tsundoku";
  const SENT_KEY = `${PREFIX}:sent`;

  // ---- sending -----------------------------------------------------------

  /** Opens the link the browser bookmark opens, with the same encoding. */
  function sendLink(link) {
    location.href = "tsundoku://add?url=" + encodeURIComponent(link);
  }

  /** What was sent in this tab, kept through its reloads (sessionStorage). */
  const sent = new Set(readSent());

  function readSent() {
    try {
      const saved = JSON.parse(sessionStorage.getItem(SENT_KEY) || "[]");
      return Array.isArray(saved) ? saved : [];
    } catch {
      return [];
    }
  }

  function remember(link) {
    sent.add(link);
    try {
      sessionStorage.setItem(SENT_KEY, JSON.stringify([...sent]));
    } catch {
      // Storage refused: the check lasts until the page reloads.
    }
  }

  // ---- the button --------------------------------------------------------

  const SVG_NS = "http://www.w3.org/2000/svg";

  function svgElement(tag, attributes, children = []) {
    const element = document.createElementNS(SVG_NS, tag);
    for (const [name, value] of Object.entries(attributes)) element.setAttribute(name, String(value));
    element.append(...children);
    return element;
  }

  /** Tsundoku's icon, three books on ai-iro, or once sent, a check on it. Built element by element, since YouTube
   * refuses HTML set as text (Trusted Types). */
  function icon(isSent) {
    const parts = [svgElement("rect", { x: 48, y: 48, width: 928, height: 928, rx: 208, fill: "#165e83" })];
    if (isSent) {
      parts.push(
        svgElement("path", {
          d: "M300 540l150 150 290-330",
          fill: "none",
          stroke: "#fff",
          "stroke-width": 110,
          "stroke-linecap": "round",
          "stroke-linejoin": "round",
        }),
      );
    } else {
      parts.push(
        svgElement("rect", { x: 216, y: 664, width: 592, height: 140, rx: 30, fill: "#f3ead7" }),
        svgElement("rect", { x: 268, y: 508, width: 540, height: 140, rx: 30, fill: "#e8b04a" }),
        svgElement("g", { transform: "rotate(-7 500 420)" }, [
          svgElement("rect", { x: 232, y: 352, width: 540, height: 140, rx: 30, fill: "#79b4da" }),
        ]),
      );
    }
    return svgElement("svg", { viewBox: "0 0 1024 1024", "aria-hidden": "true", class: `${PREFIX}-icon` }, parts);
  }

  /** What each button sends: worked out when it's clicked or refreshed, since a site may reuse its element for
   * another post. */
  const linkOf = new WeakMap();

  /** A button of the given kind (its look, in the styles below), sending what `findLink` returns. */
  function makeButton(kind, findLink) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = `${PREFIX}-button ${PREFIX}-${kind}`;
    linkOf.set(button, findLink);
    // The sites open a post when it's clicked anywhere: the button's own presses stop here.
    for (const type of ["pointerdown", "mousedown", "pointerup", "mouseup"]) {
      button.addEventListener(type, (event) => event.stopPropagation());
    }
    button.addEventListener("click", (event) => {
      event.preventDefault();
      event.stopPropagation();
      const link = findLink();
      if (!link) return;
      sendLink(link);
      remember(link);
      refresh(button);
      button.classList.add(`${PREFIX}-flash`);
      setTimeout(() => button.classList.remove(`${PREFIX}-flash`), 900);
    });
    refresh(button);
    return button;
  }

  /** Shows whether the button's post was sent: the check and "Sent to Tsundoku", or the books. */
  function refresh(button) {
    const findLink = linkOf.get(button);
    const isSent = Boolean(findLink && sent.has(findLink()));
    if (button.classList.contains(`${PREFIX}-sent`) === isSent && button.firstChild) return;
    button.classList.toggle(`${PREFIX}-sent`, isSent);
    button.title = isSent ? SENT_LABEL : LABEL;
    button.setAttribute("aria-label", button.title);
    button.replaceChildren(icon(isSent));
  }

  function refreshAll() {
    for (const button of document.querySelectorAll(`.${PREFIX}-button`)) refresh(button);
  }

  /** A holder for a button, for rows whose items are wrappers. */
  function holder(kind, button) {
    const element = document.createElement("div");
    element.className = `${PREFIX}-holder ${PREFIX}-holder-${kind}`;
    element.append(button);
    return element;
  }

  // ---- styles ------------------------------------------------------------

  const STYLES = `
    .${PREFIX}-button {
      all: unset;
      position: relative;
      box-sizing: border-box;
      display: inline-flex;
      align-items: center;
      justify-content: center;
      flex: none;
      cursor: pointer;
      border-radius: 9999px;
      transition: background-color 150ms, opacity 150ms;
    }
    .${PREFIX}-button:focus-visible { outline: 2px solid #79b4da; outline-offset: 2px; }
    .${PREFIX}-icon { display: block; width: 24px; height: 24px; }
    .${PREFIX}-flash { animation: ${PREFIX}-ring 900ms ease-out; }
    @keyframes ${PREFIX}-ring {
      from { box-shadow: 0 0 0 0 rgba(121, 180, 218, 0.9); }
      to { box-shadow: 0 0 0 10px rgba(121, 180, 218, 0); }
    }
    .${PREFIX}-holder { display: flex; align-items: center; }

    .${PREFIX}-instagram { width: 40px; height: 40px; }
    .${PREFIX}-instagram:hover { opacity: 0.6; }

    .${PREFIX}-holder-x { flex: 1 1 0; justify-content: flex-start; }
    .${PREFIX}-x { width: 34px; height: 34px; }
    .${PREFIX}-x .${PREFIX}-icon { width: 19px; height: 19px; }
    .${PREFIX}-x:hover { background-color: rgba(29, 155, 240, 0.12); }

    .${PREFIX}-youtube-watch {
      width: 36px;
      height: 36px;
      margin-left: 8px;
      background-color: var(--yt-spec-badge-chip-background, rgba(128, 128, 128, 0.12));
    }
    .${PREFIX}-youtube-watch .${PREFIX}-icon { width: 22px; height: 22px; }
    .${PREFIX}-youtube-watch:hover,
    .${PREFIX}-youtube-shorts:hover {
      background-color: var(--yt-spec-button-chip-background-hover, rgba(128, 128, 128, 0.24));
    }
    .${PREFIX}-holder-youtube-shorts { justify-content: center; margin-top: 16px; }
    .${PREFIX}-youtube-shorts {
      width: 48px;
      height: 48px;
      background-color: var(--yt-spec-badge-chip-background, rgba(128, 128, 128, 0.12));
    }
    .${PREFIX}-youtube-thumbnail {
      position: absolute;
      top: 6px;
      left: 6px;
      z-index: 1000;
      width: 30px;
      height: 30px;
      border-radius: 8px;
      background-color: rgba(0, 0, 0, 0.6);
      opacity: 0;
    }
    .${PREFIX}-youtube-thumbnail .${PREFIX}-icon { width: 22px; height: 22px; }
    .${PREFIX}-thumbnail-host:hover .${PREFIX}-youtube-thumbnail,
    .${PREFIX}-youtube-thumbnail:focus-visible,
    .${PREFIX}-youtube-thumbnail.${PREFIX}-sent { opacity: 1; }
  `;

  /** As a constructed sheet when the page allows it, since a site's rules may refuse a <style> element. */
  function addStyles() {
    try {
      const sheet = new CSSStyleSheet();
      sheet.replaceSync(STYLES);
      document.adoptedStyleSheets = [...document.adoptedStyleSheets, sheet];
      return;
    } catch {
      // Not available here (some userscript managers' sandboxes): a <style> element instead.
    }
    const style = document.createElement("style");
    style.textContent = STYLES;
    (document.head || document.documentElement).append(style);
  }

  // ---- Instagram -----------------------------------------------------------

  /** /p/ID/, /reel/ID/ or /tv/ID/, with or without the account before it (/natgeo/p/ID/). */
  const INSTAGRAM_POST = /^\/(?:[\w.]+\/)?(p|reel|tv)\/([\w-]+)/;

  function instagramLink(path) {
    const match = INSTAGRAM_POST.exec(path);
    if (!match) return null;
    const kind = match[1] === "tv" ? "p" : match[1];
    return `https://www.instagram.com/${kind}/${match[2]}/`;
  }

  /** A feed post's own link, its date's; else the page's, for a post or reel opened on its own or in a dialog. */
  function instagramPostLink(row) {
    const article = row.closest("article");
    if (article) {
      const links = [...article.querySelectorAll("a[href]")];
      const dated = links.find((link) => link.querySelector("time") && INSTAGRAM_POST.test(link.getAttribute("href")));
      const any = dated || links.find((link) => INSTAGRAM_POST.test(link.getAttribute("href")));
      if (any) return instagramLink(any.getAttribute("href"));
    }
    return instagramLink(location.pathname.replace(/^\/reels\//, "/reel/"));
  }

  /** An action icon: 24 px, in a button. Found by shape, since the labels follow the account's language. */
  const INSTAGRAM_ICON_BUTTON = '[role="button"] svg[height="24"], button svg[height="24"]';

  /** The row (or a reel's column) holding a post's Like, Comment and Share: the nearest element above an icon button
   * whose items hold one icon button each, at least three of them. A column holding a post's header (More Options),
   * its comments and its row of actions isn't one, since that row holds several. */
  function instagramActionRow(svg) {
    const button = svg.closest('[role="button"], button');
    if (!button) return null;
    let element = button.parentElement;
    for (let depth = 0; element && depth < 6; depth += 1, element = element.parentElement) {
      const icons = [...element.children]
        .map((child) => child.querySelectorAll(INSTAGRAM_ICON_BUTTON).length)
        .filter((count) => count > 0);
      if (icons.length >= 3 && icons.every((count) => count === 1)) return element;
    }
    return null;
  }

  const instagram = {
    matches: () => location.hostname === "www.instagram.com",
    scan() {
      const rows = new Set();
      for (const svg of document.querySelectorAll(`main :is(${INSTAGRAM_ICON_BUTTON})`)) {
        const row = instagramActionRow(svg);
        if (row) rows.add(row);
      }
      for (const row of rows) {
        if (row.querySelector(`.${PREFIX}-button`) || !instagramPostLink(row)) continue;
        // After Share: Like, Comment and Share come first in every layout.
        const items = [...row.children].filter((child) => child.querySelector(INSTAGRAM_ICON_BUTTON));
        const button = makeButton("instagram", () => instagramPostLink(row));
        items[2].after(holder("instagram", button));
      }
    },
  };

  // ---- X -------------------------------------------------------------------

  const X_STATUS = /^\/(\w{1,15})\/status\/(\d+)/;

  /** A post's own link, from its date (a post's page has the same link on its date); else the page's. */
  function xPostLink(article) {
    for (const time of article.querySelectorAll("a[href] time")) {
      const match = X_STATUS.exec(time.closest("a").getAttribute("href"));
      if (match) return `https://x.com/${match[1]}/status/${match[2]}`;
    }
    const match = X_STATUS.exec(location.pathname);
    return match ? `https://x.com/${match[1]}/status/${match[2]}` : null;
  }

  const x = {
    matches: () => /^(x|twitter)\.com$/.test(location.hostname),
    scan() {
      for (const article of document.querySelectorAll('article[data-testid="tweet"]')) {
        if (article.querySelector(`.${PREFIX}-button`)) continue;
        // The post's own row of actions: the last group, after any quoted post's.
        const groups = article.querySelectorAll('div[role="group"]');
        const row = groups[groups.length - 1];
        if (!row || !xPostLink(article)) continue;
        row.append(holder("x", makeButton("x", () => xPostLink(article))));
      }
    },
  };

  // ---- YouTube -------------------------------------------------------------

  /** A watch or Shorts link's video, as https://www.youtube.com/watch?v=ID or /shorts/ID. */
  function youtubeLink(href) {
    let url;
    try {
      url = new URL(href, location.origin);
    } catch {
      return null;
    }
    const short = /^\/shorts\/([\w-]{11})/.exec(url.pathname);
    if (short) return `https://www.youtube.com/shorts/${short[1]}`;
    const id = url.pathname === "/watch" ? url.searchParams.get("v") : null;
    return id && /^[\w-]{11}$/.test(id) ? `https://www.youtube.com/watch?v=${id}` : null;
  }

  /** A Short's own link, from its player's links; else the page's. */
  function youtubeShortLink(bar) {
    const reel = bar.closest("ytd-reel-video-renderer");
    const link = reel && reel.querySelector('a[href*="/shorts/"]');
    return (link && youtubeLink(link.getAttribute("href"))) || youtubeLink(location.href);
  }

  /** Thumbnails link to a video and hold its picture: in feeds, a channel's videos, search results and the list beside
   * a video. */
  const YOUTUBE_THUMBNAILS = 'a[href^="/watch?v="], a[href^="/shorts/"]';
  const YOUTUBE_NOT_THUMBNAILS = "#movie_player, ytd-reel-video-renderer, ytd-watch-metadata";
  /** What makes a link a thumbnail: its picture (before or after it loads), or, over a thumbnail whose video plays on
   * hover, the preview YouTube lays on top of it, which would hide the thumbnail's own button. */
  const YOUTUBE_PICTURES = "img, yt-image, yt-thumbnail-view-model";
  const YOUTUBE_PREVIEW = "ytd-video-preview";

  const youtube = {
    matches: () => location.hostname === "www.youtube.com",
    scan() {
      // A watch page: after Share. YouTube keeps the row from one video to the next, so the link is the page's.
      const row = document.querySelector("ytd-watch-metadata #top-level-buttons-computed");
      if (row && !row.querySelector(`.${PREFIX}-button`)) {
        row.append(makeButton("youtube-watch", () => youtubeLink(location.href)));
      }
      // Shorts: each Short's column of actions, before its sound.
      for (const bar of document.querySelectorAll("reel-action-bar-view-model")) {
        if (bar.querySelector(`.${PREFIX}-button`)) continue;
        const button = holder("youtube-shorts", makeButton("youtube-shorts", () => youtubeShortLink(bar)));
        const sound = bar.querySelector(":scope > pivot-button-view-model");
        if (sound) sound.before(button);
        else bar.append(button);
      }
      // Thumbnails: a corner button, shown on hover, or always once sent.
      for (const anchor of document.querySelectorAll(YOUTUBE_THUMBNAILS)) {
        if (anchor.querySelector(`.${PREFIX}-button`)) continue;
        if (!anchor.querySelector(YOUTUBE_PICTURES) && !anchor.closest(YOUTUBE_PREVIEW)) continue;
        // Hidden ones (a collapsed list) wait until they show. Not the player's own, a Short's, or links in a video's
        // description.
        if (anchor.getBoundingClientRect().width < 80 || anchor.closest(YOUTUBE_NOT_THUMBNAILS)) continue;
        if (getComputedStyle(anchor).position === "static") anchor.style.position = "relative";
        anchor.classList.add(`${PREFIX}-thumbnail-host`);
        anchor.append(makeButton("youtube-thumbnail", () => youtubeLink(anchor.getAttribute("href"))));
      }
    },
  };

  // ---- running -------------------------------------------------------------

  const site = [instagram, x, youtube].find((candidate) => candidate.matches());
  if (!site) return;
  addStyles();

  /** Scans at most four times a second, however often the page changes. */
  let scheduled = false;
  function schedule() {
    if (scheduled) return;
    scheduled = true;
    setTimeout(() => {
      scheduled = false;
      site.scan();
      refreshAll();
    }, 250);
  }

  new MutationObserver(schedule).observe(document.documentElement, { childList: true, subtree: true });
  // Moving to another page without reloading: YouTube says so; the others change the address.
  for (const type of ["yt-navigate-finish", "popstate"]) window.addEventListener(type, schedule);
  schedule();
})();

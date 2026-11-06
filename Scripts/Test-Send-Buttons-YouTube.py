"""Checks the send buttons' userscript on YouTube, signed out (it needs the network). It loads a watch page, the list
beside it, a channel's videos, search results and Shorts with the script injected, and checks the buttons show once
each, through scrolling and moving between videos without reloading, and send the right tsundoku:// link without
leaving the page; DevTools reports each link the page tries to open. Run it with Python Playwright:
python Scripts\Test-Send-Buttons-YouTube.py [light|dark]. Screenshots go to the temp folder's tsundoku-send-buttons.
"""
import json
import re
import sys
import tempfile
from pathlib import Path
from urllib.parse import unquote

from playwright.sync_api import sync_playwright

SCRIPT = (Path(__file__).resolve().parent.parent / "tsundoku" / "userscripts" / "tsundoku-send-buttons.user.js").read_text(encoding="utf-8")
SHOTS = Path(tempfile.gettempdir()) / "tsundoku-send-buttons"
SHOTS.mkdir(exist_ok=True)
scheme = sys.argv[1] if len(sys.argv) > 1 else "light"
failures = []


def check(condition, what):
    print(("PASS " if condition else "FAIL ") + what)
    if not condition:
        failures.append(what)


COUNTS = """() => {
  const thumbs = [...document.querySelectorAll('a[href^="/watch?v="], a[href^="/shorts/"]')]
    .filter(a => a.querySelector('img, yt-image, yt-thumbnail-view-model') && !a.closest('ytd-video-preview') && a.getBoundingClientRect().width >= 80 && !a.closest('#movie_player, ytd-reel-video-renderer, ytd-watch-metadata'));
  return {
    watch: document.querySelectorAll('ytd-watch-metadata #top-level-buttons-computed .tsundoku-send-button').length,
    shorts: [...document.querySelectorAll('reel-action-bar-view-model')].map(b => b.querySelectorAll('.tsundoku-send-button').length),
    thumbs: thumbs.length,
    thumbButtons: thumbs.filter(a => a.querySelectorAll(':scope > .tsundoku-send-button').length === 1).length,
    doubled: [...document.querySelectorAll('a, ytd-watch-metadata #top-level-buttons-computed, reel-action-bar-view-model')].filter(e => e.querySelectorAll(':scope > .tsundoku-send-button, :scope > .tsundoku-send-holder > .tsundoku-send-button').length > 1).length,
  };
}"""

with sync_playwright() as p:
    browser = p.chromium.launch()
    context = browser.new_context(viewport={"width": 1400, "height": 900}, locale="en-US", color_scheme=scheme)
    context.add_cookies([{"name": "SOCS", "value": "CAI", "domain": ".youtube.com", "path": "/"}])
    if scheme == "dark":
        context.add_cookies([{"name": "PREF", "value": "f6=400&hl=en", "domain": ".youtube.com", "path": "/"}])
    page = context.new_page()
    sent = []

    def on_console(message):
        found = re.search(r"tsundoku://add\?url=([^'\"\s]+)", message.text)
        if found:
            sent.append(unquote(found.group(1)))

    cdp = context.new_cdp_session(page)
    cdp.send("Page.enable")
    cdp.on("Page.frameRequestedNavigation", lambda event: on_console(type("Message", (), {"text": event.get("url", "")})))
    page.on("pageerror", lambda error: failures.append(f"page error: {error}"))

    def load(url):
        page.goto(url, wait_until="domcontentloaded")
        page.wait_for_timeout(5000)
        page.evaluate(SCRIPT)
        page.wait_for_timeout(1500)

    def last_sent(before):
        page.wait_for_timeout(800)
        return sent[-1] if len(sent) > before else None

    # 1. A watch page: one button after Share, and thumbnails in the side list.
    load("https://www.youtube.com/watch?v=jNQXAC9IVRw")
    counts = page.evaluate(COUNTS)
    print(json.dumps(counts))
    check(counts["watch"] == 1, "watch page has one button")
    check(counts["thumbs"] > 5 and counts["thumbButtons"] == counts["thumbs"], "every side-list thumbnail has one button")
    page.locator("ytd-watch-metadata").screenshot(path=str(SHOTS / f"watch-{scheme}.png"))
    before = len(sent)
    page.locator("ytd-watch-metadata .tsundoku-send-youtube-watch").click()
    check(last_sent(before) == "https://www.youtube.com/watch?v=jNQXAC9IVRw", "watch button sends the video")
    check("watch?v=jNQXAC9IVRw" in page.url, "the page stays")
    check(page.locator("ytd-watch-metadata .tsundoku-send-youtube-watch.tsundoku-send-sent").count() == 1, "watch button shows sent")
    page.wait_for_timeout(200)
    page.locator("ytd-watch-metadata").screenshot(path=str(SHOTS / f"watch-sent-{scheme}.png"))

    # Scrolling loads more of the side list.
    for _ in range(4):
        page.mouse.wheel(0, 2500)
        page.wait_for_timeout(1200)
    counts = page.evaluate(COUNTS)
    print(json.dumps(counts))
    check(counts["thumbButtons"] == counts["thumbs"] and counts["doubled"] == 0, "after scrolling: one button per thumbnail, none doubled")

    # A side-list thumbnail's button: sends that video, doesn't open it.
    page.mouse.wheel(0, -20000)
    page.wait_for_timeout(800)
    thumb = page.locator("yt-lockup-view-model a.tsundoku-send-thumbnail-host, ytd-compact-video-renderer a.tsundoku-send-thumbnail-host").first.element_handle()
    href = thumb.get_attribute("href")
    thumb.hover()
    page.wait_for_timeout(400)
    page.screenshot(path=str(SHOTS / f"side-hover-{scheme}.png"), clip={"x": 900, "y": 0, "width": 500, "height": 500})
    before = len(sent)
    spot = thumb.eval_on_selector(".tsundoku-send-button", "b => { const r = b.getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2] }")
    page.wait_for_timeout(2500)
    on_top = page.evaluate("([x, y]) => { const b = document.elementFromPoint(x, y)?.closest('.tsundoku-send-button'); return b ? (b.closest('ytd-video-preview') ? 'preview' : 'thumbnail') : null }", spot)
    print("side list, on top:", on_top)
    check(on_top is not None, f"side list: a send button is on top of the hovered thumbnail ({on_top})")
    page.mouse.click(*spot)
    expected = "https://www.youtube.com/watch?v=" + re.search(r"v=([\w-]{11})", href).group(1)
    check(last_sent(before) == expected, f"thumbnail button sends its video ({expected})")
    page.wait_for_timeout(1000)
    check("watch?v=jNQXAC9IVRw" in page.url, "the thumbnail's button didn't open the video")

    # 2. In-page navigation: open the next video from the side list.
    target = page.locator("yt-lockup-view-model a[href^='/watch?v=']:not(.tsundoku-send-thumbnail-host)").nth(1)
    target_href = target.get_attribute("href")
    target.click()
    page.wait_for_timeout(5000)
    counts = page.evaluate(COUNTS)
    print(page.url, json.dumps(counts))
    check(counts["watch"] == 1, "after in-page navigation: still one watch button")
    new_id = re.search(r"v=([\w-]{11})", page.url).group(1)
    before = len(sent)
    page.locator("ytd-watch-metadata .tsundoku-send-youtube-watch").click()
    check(last_sent(before) == f"https://www.youtube.com/watch?v={new_id}", "after navigation the watch button sends the new video")

    # 3. A channel's videos (a feed-like grid) and search results.
    for url, name in [("https://www.youtube.com/@Blender/videos", "channel"), ("https://www.youtube.com/results?search_query=big+buck+bunny", "search")]:
        load(url)
        for _ in range(3):
            page.mouse.wheel(0, 3000)
            page.wait_for_timeout(1500)
        page.wait_for_timeout(1000)
        counts = page.evaluate(COUNTS)
        print(name, json.dumps(counts))
        if counts["thumbButtons"] != counts["thumbs"]:
            print(page.evaluate("""() => [...document.querySelectorAll('a[href^="/watch?v="], a[href^="/shorts/"]')].filter(a => a.querySelector('img, yt-image, yt-thumbnail-view-model') && !a.querySelector('.tsundoku-send-button')).map(a => a.className + ' w=' + Math.round(a.getBoundingClientRect().width) + ' ' + a.parentElement.tagName)"""))
        check(counts["thumbs"] > 5 and counts["thumbButtons"] == counts["thumbs"] and counts["doubled"] == 0, f"{name}: one button per thumbnail after scrolling")
        page.mouse.wheel(0, -20000)
        page.wait_for_timeout(800)
        host = page.locator("ytd-video-renderer a.tsundoku-send-thumbnail-host, yt-lockup-view-model a.tsundoku-send-thumbnail-host").first.element_handle()
        # Away from the sticky header: the results above can still shift as they load.
        for _ in range(2):
            host.evaluate("a => a.scrollIntoView({block: 'center'})")
            page.wait_for_timeout(1000)
        box = host.bounding_box()
        page.mouse.move(box["x"] + box["width"] / 2, box["y"] + box["height"] / 2)
        page.wait_for_timeout(3000)
        page.screenshot(path=str(SHOTS / f"{name}-hover-{scheme}.png"), clip={"x": 0, "y": 0, "width": 1400, "height": 600})
        # Whatever is on top where the thumbnail's button sits: its own, or the one on YouTube's hover preview.
        spot = host.eval_on_selector(".tsundoku-send-button", "b => { const r = b.getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2] }")
        on_top = page.evaluate("([x, y]) => { const top = document.elementFromPoint(x, y); const b = top && top.closest('.tsundoku-send-button'); return b ? (b.closest('ytd-video-preview') ? 'preview' : 'thumbnail') : (top ? top.tagName + '.' + top.className : null) }", spot)
        print(name, "on top at the button's spot:", on_top)
        check(on_top in ("preview", "thumbnail"), f"{name}: a send button is on top of the hovered thumbnail ({on_top})")
        before = len(sent)
        url_before = page.url
        page.mouse.click(*spot)
        got = last_sent(before)
        host_href = host.get_attribute("href")
        check(got is not None and got.split("youtube.com")[1].replace("/watch?v=", "") in host_href, f"{name}: the button sends the hovered video ({got}, {host_href})")
        page.wait_for_timeout(800)
        check(page.url == url_before, f"{name}: the page stays")

    # 4. Shorts: a button in the column, and the next Short gets its own.
    load("https://www.youtube.com/shorts/D2kjeaX1T_w")
    counts = page.evaluate(COUNTS)
    print("shorts", json.dumps(counts))
    check(counts["shorts"] and all(n == 1 for n in counts["shorts"]), "each Short's column has one button")
    page.locator("reel-action-bar-view-model").first.screenshot(path=str(SHOTS / f"shorts-{scheme}.png"))
    before = len(sent)
    page.locator("reel-action-bar-view-model .tsundoku-send-button").first.click()
    check(last_sent(before) == "https://www.youtube.com/shorts/D2kjeaX1T_w", "Shorts button sends the Short")
    page.keyboard.press("ArrowDown")
    page.wait_for_timeout(4000)
    counts = page.evaluate(COUNTS)
    print("next short", page.url, json.dumps(counts))
    check(all(n == 1 for n in counts["shorts"]), "after the next Short: one button per column")
    next_id = re.search(r"/shorts/([\w-]{11})", page.url)
    if next_id and next_id.group(1) != "D2kjeaX1T_w":
        active = page.locator("ytd-reel-video-renderer[is-active] reel-action-bar-view-model .tsundoku-send-button, reel-action-bar-view-model .tsundoku-send-button").last
        before = len(sent)
        active.click()
        got = last_sent(before)
        check(got == f"https://www.youtube.com/shorts/{next_id.group(1)}", f"next Short's button sends it ({got})")

    browser.close()

print("FAILURES:", failures if failures else "none")
sys.exit(1 if failures else 0)

"""Checks the send buttons' userscript on Instagram and X (it needs the network). Instagram's public post and reel pages
render signed out, so they're tested live; its feed and X need sign-in, so they're tested on markup shaped like theirs
(roles, labels in French, link shapes, data-testid), served at their own addresses so the script's site check passes.
Run it with Python Playwright: python Scripts\Test-Send-Buttons-Instagram-X.py.
"""
import re
import sys
import tempfile
from pathlib import Path
from urllib.parse import unquote

from playwright.sync_api import sync_playwright

SCRIPT = (Path(__file__).resolve().parent.parent / "tsundoku" / "userscripts" / "tsundoku-send-buttons.user.js").read_text(encoding="utf-8")
SHOTS = Path(tempfile.gettempdir()) / "tsundoku-send-buttons"
SHOTS.mkdir(exist_ok=True)
failures = []


def check(condition, what):
    print(("PASS " if condition else "FAIL ") + what)
    if not condition:
        failures.append(what)


def icon_button(label):
    return f'<span><div><div role="button" tabindex="0"><div><span><svg aria-label="{label}" height="24" width="24" viewBox="0 0 24 24"><path d="M2 2h20v20H2z"/></svg></span></div></div></div></span>'


def instagram_post(post_id, account):
    return f"""
<article>
  <header><a href="/{account}/">{account}</a> • <a href="/p/{post_id}/"><time datetime="2026-11-05T10:00:00Z">1 j</time></a></header>
  <div class="media"><a href="/p/{post_id}/c/123/">comment link</a><img src="data:," width="400" height="400"></div>
  <section><div class="row">{icon_button("J’aime")}{icon_button("Commenter")}{icon_button("Partager")}<div class="spacer"></div>{icon_button("Enregistrer")}</div></section>
</article>"""


INSTAGRAM_FEED = f"""<!doctype html><html><body>
<div class="nav">{''.join(icon_button(n) for n in ["Accueil", "Rechercher", "Explorer", "Reels", "Messages"])}</div>
<main id="feed">{instagram_post("ABC123", "natgeo")}{instagram_post("DEF456", "nasa")}</main>
<script>document.addEventListener("click", e => {{ if (e.target.closest("article")) window.openedPost = true }})</script>
</body></html>"""


def x_post(user, status, quoted=None):
    quote = ""
    if quoted:
        quote = f'<div role="link"><a href="/{quoted[0]}/status/{quoted[1]}"><time>3h</time></a><span>quoted text</span></div>'
    group = "".join(f'<div><button data-testid="{n}"><svg viewBox="0 0 24 24" width="19"><path d="M1 1h22v22H1z"/></svg></button></div>' for n in ["reply", "retweet", "like", "bookmark"])
    return f"""
<article data-testid="tweet" tabindex="0">
  <a href="/{user}">@{user}</a> · <a href="/{user}/status/{status}"><time datetime="2026-11-05T10:00:00Z">5h</time></a>
  <div lang="fr">texte</div>{quote}
  <div role="group" aria-label="actions" style="display:flex;justify-content:space-between;max-width:425px">{group}</div>
</article>"""


X_HOME = f"""<!doctype html><html><body style="background:#000;color:#e7e9ea">
<main><section id="timeline">{x_post("NASA", "1234567890123456789")}{x_post("jack", "20", ("NASA", "999"))}</section></main>
<script>document.addEventListener("click", e => {{ if (e.target.closest("article")) window.openedPost = true }})</script>
</body></html>"""

with sync_playwright() as p:
    browser = p.chromium.launch()
    context = browser.new_context(viewport={"width": 1280, "height": 900}, locale="en-US")
    page = context.new_page()
    sent = []
    cdp = context.new_cdp_session(page)
    cdp.send("Page.enable")

    def on_navigation(event):
        found = re.match(r"tsundoku://add\?url=(.+)$", event.get("url", ""))
        if found:
            sent.append(unquote(found.group(1)))

    cdp.on("Page.frameRequestedNavigation", on_navigation)
    page.on("pageerror", lambda error: failures.append(f"page error: {error}"))

    def click_and_read(locator):
        before = len(sent)
        locator.scroll_into_view_if_needed()
        box = locator.bounding_box()
        x, y = box["x"] + box["width"] / 2, box["y"] + box["height"] / 2
        on_top = page.evaluate("([x, y]) => Boolean(document.elementFromPoint(x, y)?.closest('.tsundoku-send-button'))", [x, y])
        if not on_top:
            print("  (something covers the button here)")
        page.mouse.click(x, y)
        page.wait_for_timeout(700)
        return sent[-1] if len(sent) > before else None

    # --- Instagram, live: a post and a reel on their own pages, signed out.
    for url, expected in [("https://www.instagram.com/p/DeuOR-CjKE9/", "https://www.instagram.com/p/DeuOR-CjKE9/"),
                          ("https://www.instagram.com/reel/DfAWxdIFmVB/", "https://www.instagram.com/reel/DfAWxdIFmVB/")]:
        page.goto(url, wait_until="domcontentloaded")
        page.wait_for_timeout(6000)
        decline = page.get_by_role("button", name="Decline optional cookies")
        if decline.count():
            decline.first.click()
            page.wait_for_timeout(1500)
        page.evaluate(SCRIPT)
        page.wait_for_timeout(1500)
        # Signed out, a prompt to sign up covers the page: closed as a visitor would.
        close = page.locator('div[role="dialog"] svg[aria-label="Close"]')
        if close.count():
            close.first.click()
            page.wait_for_timeout(800)
        buttons = page.locator(".tsundoku-send-button")
        count = buttons.count()
        check(count == 1, f"Instagram live {url}: one button ({count})")
        if count:
            order = page.evaluate("""() => { const b = document.querySelector('.tsundoku-send-holder'); const prev = b.previousElementSibling;
                return prev ? prev.querySelector('svg[aria-label]')?.getAttribute('aria-label') : null }""")
            check(order == "Share", f"Instagram live: the button comes after Share ({order})")
            row = page.locator(".tsundoku-send-holder").first.locator("xpath=..")
            row.screenshot(path=str(SHOTS / f"instagram-live-{'reel' if 'reel' in url else 'post'}.png"))
            got = click_and_read(buttons.first)
            check(got == expected, f"Instagram live: sends {got}")
            check(page.url.startswith(url), "Instagram live: the page stays")

    # --- Instagram, synthetic feed (French labels), with a post loaded later.
    page.route("https://www.instagram.com/", lambda route: route.fulfill(body=INSTAGRAM_FEED, content_type="text/html"))
    page.goto("https://www.instagram.com/")
    page.evaluate(SCRIPT)
    page.wait_for_timeout(800)
    check(page.locator("main .tsundoku-send-button").count() == 2, "Instagram feed: one button per post")
    check(page.locator(".nav .tsundoku-send-button").count() == 0, "Instagram feed: none in the navigation")
    after_share = page.evaluate("""() => [...document.querySelectorAll('.tsundoku-send-holder')].map(h => h.previousElementSibling.querySelector('svg').getAttribute('aria-label'))""")
    check(after_share == ["Partager", "Partager"], f"Instagram feed: after Share in any language ({after_share})")
    got = click_and_read(page.locator("main article").nth(1).locator(".tsundoku-send-button"))
    check(got == "https://www.instagram.com/p/DEF456/", f"Instagram feed: the second post sends its own link ({got})")
    check(not page.evaluate("window.openedPost === true"), "Instagram feed: the click doesn't reach the post")
    page.evaluate(f"document.querySelector('#feed').insertAdjacentHTML('beforeend', {instagram_post('GHI789', 'bbc')!r})")
    page.wait_for_timeout(800)
    check(page.locator("main .tsundoku-send-button").count() == 3, "Instagram feed: a post loaded later gets one")
    page.evaluate("document.querySelector('#feed').append(document.createElement('div'))")
    page.wait_for_timeout(800)
    check(page.locator("main .tsundoku-send-button").count() == 3, "Instagram feed: nothing doubled on later changes")
    check(page.locator("main article").nth(1).locator(".tsundoku-send-sent").count() == 1, "Instagram feed: the sent post keeps its check")

    # --- X, synthetic timeline, then a post's page reached without reloading.
    page.route("https://x.com/home", lambda route: route.fulfill(body=X_HOME, content_type="text/html"))
    page.goto("https://x.com/home")
    page.evaluate(SCRIPT)
    page.wait_for_timeout(800)
    check(page.locator("article .tsundoku-send-button").count() == 2, "X: one button per post")
    check(page.locator('div[role="group"] > .tsundoku-send-holder .tsundoku-send-button').count() == 2, "X: in the row of actions")
    page.locator("article").first.screenshot(path=str(SHOTS / "x-synthetic.png"))
    got = click_and_read(page.locator("article").nth(1).locator(".tsundoku-send-button"))
    check(got == "https://x.com/jack/status/20", f"X: a post quoting another sends its own link ({got})")
    check(not page.evaluate("window.openedPost === true"), "X: the click doesn't open the post")
    page.evaluate(f"document.querySelector('#timeline').insertAdjacentHTML('beforeend', {x_post('NASA', '42')!r})")
    page.wait_for_timeout(800)
    check(page.locator("article .tsundoku-send-button").count() == 3, "X: a post loaded later gets one")
    page.evaluate(f"""history.pushState(null, '', '/NASA/status/1234567890123456789');
        document.querySelector('#timeline').innerHTML = {x_post('NASA', '1234567890123456789')!r}""")
    page.wait_for_timeout(800)
    check(page.locator("article .tsundoku-send-button").count() == 1, "X: a post's page has one button")
    check(page.locator(".tsundoku-send-sent").count() == 0, "X: a post not sent shows the books")

    browser.close()

print("FAILURES:", failures if failures else "none")
sys.exit(1 if failures else 0)

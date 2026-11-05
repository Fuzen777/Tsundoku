"""Trims gallery-dl's raw answers in tsundoku/core/tests/fixtures/gallery to what the tests read.

gallery-dl's `-j` answers carry everything it knows, including fields about the account that fetched them (whether it
liked or follows the post's author) and CDN links signed for that request. This keeps only the keys gallery.rs reads,
and cuts the query string off every link. Run it after capturing new answers:

    python Scripts\\Trim-Gallery-Fixtures.py
"""

import json
import pathlib

FOLDER = pathlib.Path(__file__).resolve().parent.parent / "tsundoku" / "core" / "tests" / "fixtures" / "gallery"

# The keys gallery.rs reads, on posts and files alike, plus a few that say what the file is.
KEPT_KEYS = {
    "category", "subcategory", "post_shortcode", "shortcode", "post_id", "tweet_id", "id", "index", "post_url",
    "username", "fullname", "author", "description", "content", "date", "count", "num", "extension",
    "display_url", "width", "height", "type", "error", "message",
}
KEPT_AUTHOR_KEYS = {"name", "nick"}


def strip_query(value):
    return value.split("?", 1)[0] if isinstance(value, str) and value.startswith("http") else value


def trim_metadata(metadata):
    if not isinstance(metadata, dict):
        return metadata
    trimmed = {}
    for key, value in metadata.items():
        if key not in KEPT_KEYS:
            continue
        if key == "author" and isinstance(value, dict):
            value = {name: value[name] for name in KEPT_AUTHOR_KEYS if name in value}
        trimmed[key] = strip_query(value)
    return trimmed


def trim_message(message):
    if not isinstance(message, list) or not message:
        return message
    kind = message[0]
    if kind in (3, 6) and len(message) == 3:
        return [kind, strip_query(message[1]), trim_metadata(message[2])]
    return [kind] + [trim_metadata(part) for part in message[1:]]


for path in sorted(FOLDER.glob("*.json")):
    messages = json.loads(path.read_text(encoding="utf-8"))
    trimmed = [trim_message(message) for message in messages]
    path.write_text(json.dumps(trimmed, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\r\n")
    print(f"{path.name}: {len(trimmed)} messages")

//! The links inside a post (a Fanbox or a Patreon post's), which go to Pending, read as links a page sent (the app
//! crate's `PendingLinks::add_from_page`): once the post is downloaded (Fuzen's choice), or at once when the post has
//! nothing of its own to download.

use std::sync::OnceLock;

/// Where the links go.
type LinkSink = Box<dyn Fn(Vec<String>) + Send + Sync>;

static SINK: OnceLock<LinkSink> = OnceLock::new();

/// Gives where the links inside a post go, once, at startup.
pub fn on_post_links(sink: LinkSink) {
    let _ = SINK.set(sink);
}

/// Sends a post's links to Pending, if it has any.
pub fn send(links: &[String]) {
    if let (false, Some(sink)) = (links.is_empty(), SINK.get()) {
        sink(links.to_vec());
    }
}

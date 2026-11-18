//! An X account's Media tab, as X serves it since gallery-dl 1.32.20 follows it: two lists, the account's posts with
//! photos and its posts with videos, each newest first. gallery-dl reads the tab as one list after the other, every
//! photo post before the first video post, so the newest 50 of an account with many photos held no video, and a
//! watched account never met a new one. Each list is read on its own instead (`?filter=photos`, `?filter=videos`), both
//! at once, and the two are merged by date (`merge`). Each keeps its own place for the next read (`MediaPlaces`).
//! Neither list holds animated GIFs: posts with only a GIF can't be listed.

use std::collections::HashSet;
use std::thread;
use std::time::Duration;

use super::MediaPlaces;
use crate::cancel::CancelToken;
use crate::gallery::{self, AccountInfo, GalleryTools};
use crate::metadata::{PreviewError, VideoInfo};

/// One of the Media tab's two lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MediaList {
    Photos,
    Videos,
}

impl MediaList {
    /// Photos first: a post in both lists is kept as the photos' list gave it.
    const BOTH: [Self; 2] = [Self::Photos, Self::Videos];

    /// The list's page: the Media tab's (`listing_link`'s `x.com/<name>/media`) with gallery-dl's filter for it.
    fn link(self, media_tab: &str) -> String {
        let filter = match self {
            Self::Photos => "photos",
            Self::Videos => "videos",
        };
        format!("{media_tab}?filter={filter}")
    }

    /// Where the list was read up to, or `None` once it ended.
    fn place(self, places: MediaPlaces) -> Option<u32> {
        match self {
            Self::Photos => places.photos,
            Self::Videos => places.videos,
        }
    }
}

/// Where both lists start: neither read yet.
pub(super) const FROM_THE_START: MediaPlaces = MediaPlaces {
    photos: Some(0),
    videos: Some(0),
};

/// What one read of a list answered.
#[derive(Debug, Clone, PartialEq)]
struct ListAnswer {
    /// Its posts with files, in the list's order.
    posts: Vec<VideoInfo>,
    /// How many posts it answered, those without files included, as `--post-range` counts them.
    answered: u32,
    /// The list ended within this read.
    ended: bool,
    account: Option<AccountInfo>,
}

/// Both lists' posts after `MediaPlaces`, merged.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Merged {
    /// Newest first, each post once.
    pub posts: Vec<VideoInfo>,
    /// Where the next read starts, or `None` once both lists ended and every post read was handed over.
    pub next: Option<MediaPlaces>,
    /// The account, from whichever list gave it.
    pub account: Option<AccountInfo>,
}

/// Reads both lists from `from`, each at most `count` posts or all the rest, both at once, and merges them. Only the
/// posts newer than where a list stopped can be merged in order (the other list's older ones may come before that
/// list's next ones), so the rest are left for the next read. With `keep_at_most`, only that many of the newest are
/// kept, as many as one list read would give.
pub(super) fn read(
    images: &GalleryTools,
    media_tab: &str,
    from: MediaPlaces,
    count: Option<u32>,
    keep_at_most: Option<usize>,
    timeout: Duration,
    cancel: &CancelToken,
) -> Result<Merged, PreviewError> {
    let answers = thread::scope(|scope| {
        let reads = MediaList::BOTH.map(|list| {
            list.place(from).map(|place| {
                let link = list.link(media_tab);
                scope.spawn(move || read_list(images, &link, place, count, timeout, cancel))
            })
        });
        reads.map(|read| {
            read.map(|handle| {
                handle.join().unwrap_or_else(|_| {
                    Err(PreviewError::Failed {
                        detail: "reading the account's posts stopped unexpectedly".to_owned(),
                    })
                })
            })
        })
    });
    let [photos, videos] = answers;
    let answers = [photos.transpose()?, videos.transpose()?];
    Ok(merge(from, answers, keep_at_most))
}

/// One list's posts after `place`: at most `count`, or all the rest.
fn read_list(
    images: &GalleryTools,
    link: &str,
    place: u32,
    count: Option<u32>,
    timeout: Duration,
    cancel: &CancelToken,
) -> Result<ListAnswer, PreviewError> {
    let range = match count {
        Some(count) => format!("{}-{}", place + 1, place + count),
        None => format!("{}-", place + 1),
    };
    let answer = gallery::run(images, link, &["--post-range", &range], timeout, cancel)?;
    let found = gallery::parse_posts(&answer, link)?;
    let answered = u32::try_from(found.answered).unwrap_or(u32::MAX);
    Ok(ListAnswer {
        posts: found.posts,
        answered,
        ended: count.is_none_or(|count| answered < count),
        account: found.account,
    })
}

/// A post's place in time: its ID, which X gives in the order posts are made. One without a numeric ID goes last.
fn order(post: &VideoInfo) -> u64 {
    post.video_id.parse().unwrap_or(0)
}

/// Merges what each list read after `from` gave (`None` for a list not read, having ended before): its posts down to
/// the newest place where a list that goes on stopped, newest first, each post once (as the photos' list gave it), at
/// most `keep_at_most` of them. Each list then starts after the posts of its own answer that were kept, counted in the
/// answer's order, so a post left out is read again rather than skipped.
fn merge(
    from: MediaPlaces,
    answers: [Option<ListAnswer>; 2],
    keep_at_most: Option<usize>,
) -> Merged {
    // A list that goes on may still hold posts newer than the other list's oldest: merge only down to its own oldest.
    let mut down_to = answers
        .iter()
        .flatten()
        .filter(|answer| !answer.ended)
        .filter_map(|answer| answer.posts.iter().map(order).min())
        .max()
        .unwrap_or(0);

    let mut listed = HashSet::new();
    let mut posts: Vec<&VideoInfo> = answers
        .iter()
        .flatten()
        .flat_map(|answer| &answer.posts)
        .filter(|post| order(post) >= down_to && listed.insert(post.video_id.as_str()))
        .collect();
    posts.sort_by_key(|post| std::cmp::Reverse(order(post)));
    if let Some(keep) = keep_at_most.filter(|keep| posts.len() > *keep) {
        posts.truncate(keep.max(1));
        down_to = posts.last().map_or(down_to, |last| order(last));
    }

    let next_place = |list: MediaList, answer: &Option<ListAnswer>| {
        let place = list.place(from)?;
        let Some(answer) = answer else {
            return Some(place);
        };
        let kept = answer
            .posts
            .iter()
            .take_while(|post| order(post) >= down_to)
            .count();
        if kept < answer.posts.len() {
            Some(place + u32::try_from(kept).unwrap_or(u32::MAX))
        } else if answer.ended {
            None
        } else {
            Some(place + answer.answered)
        }
    };
    let [photos, videos] = &answers;
    let next = MediaPlaces {
        photos: next_place(MediaList::Photos, photos),
        videos: next_place(MediaList::Videos, videos),
    };
    Merged {
        posts: posts.into_iter().cloned().collect(),
        next: (next.photos.is_some() || next.videos.is_some()).then_some(next),
        account: answers
            .iter()
            .flatten()
            .find_map(|answer| answer.account.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A post whose ID is `id`.
    fn post(id: u64) -> VideoInfo {
        VideoInfo {
            site: "twitter".to_owned(),
            site_name: "x.com".to_owned(),
            video_id: id.to_string(),
            url: format!("https://x.com/someone/status/{id}"),
            title: String::new(),
            uploader: None,
            account: None,
            duration_seconds: None,
            upload_date: None,
            thumbnail_url: None,
            best_height: None,
            post: None,
            files: None,
            post_files: Vec::new(),
            shared_files: Vec::new(),
            playlist: None,
        }
    }

    /// An answer of these posts, of `answered` in all, which ended or not.
    fn answer(ids: &[u64], answered: u32, ended: bool) -> Option<ListAnswer> {
        Some(ListAnswer {
            posts: ids.iter().copied().map(post).collect(),
            answered,
            ended,
            account: None,
        })
    }

    fn ids(merged: &Merged) -> Vec<u64> {
        merged.posts.iter().map(order).collect()
    }

    fn places(photos: Option<u32>, videos: Option<u32>) -> Option<MediaPlaces> {
        Some(MediaPlaces { photos, videos })
    }

    #[test]
    fn reads_each_list_from_its_own_page() {
        assert_eq!(
            MediaList::Photos.link("https://x.com/NASA/media"),
            "https://x.com/NASA/media?filter=photos"
        );
        assert_eq!(
            MediaList::Videos.link("https://x.com/NASA/media"),
            "https://x.com/NASA/media?filter=videos"
        );
    }

    #[test]
    fn merges_photos_and_videos_by_date() {
        // Both lists ended: every post, newest first.
        let merged = merge(
            FROM_THE_START,
            [answer(&[90, 70, 50], 3, true), answer(&[80, 60], 2, true)],
            None,
        );
        assert_eq!(ids(&merged), [90, 80, 70, 60, 50]);
        assert_eq!(merged.next, None, "nothing more");
    }

    #[test]
    fn stops_where_a_list_that_goes_on_stopped() {
        // The photos go on past 70: a video older than that may have photos between it and 70 still unread.
        let merged = merge(
            FROM_THE_START,
            [
                answer(&[90, 80, 70], 3, false),
                answer(&[85, 40, 10], 3, true),
            ],
            None,
        );
        assert_eq!(ids(&merged), [90, 85, 80, 70]);
        assert_eq!(
            merged.next,
            places(Some(3), Some(1)),
            "the photos after their 3, the videos after the one kept"
        );

        // Both go on: down to the newer of the two places where they stopped.
        let merged = merge(
            MediaPlaces {
                photos: Some(50),
                videos: Some(20),
            },
            [
                answer(&[90, 80, 70], 3, false),
                answer(&[85, 75, 60], 3, false),
            ],
            None,
        );
        assert_eq!(ids(&merged), [90, 85, 80, 75, 70]);
        assert_eq!(merged.next, places(Some(53), Some(22)));
    }

    #[test]
    fn keeps_the_newest_it_was_asked_for() {
        let merged = merge(
            FROM_THE_START,
            [
                answer(&[90, 70, 50, 30], 4, true),
                answer(&[80, 60], 2, true),
            ],
            Some(3),
        );
        assert_eq!(ids(&merged), [90, 80, 70]);
        assert_eq!(
            merged.next,
            places(Some(2), Some(1)),
            "each list starts after the posts it gave that were kept"
        );
    }

    #[test]
    fn a_list_that_ended_isnt_read_again() {
        // The videos ended before: only the photos are read.
        let merged = merge(
            MediaPlaces {
                photos: Some(50),
                videos: None,
            },
            [answer(&[40, 30], 2, false), None],
            None,
        );
        assert_eq!(ids(&merged), [40, 30]);
        assert_eq!(merged.next, places(Some(52), None));

        // An empty list ends at once.
        let merged = merge(
            FROM_THE_START,
            [answer(&[40, 30], 2, true), answer(&[], 0, true)],
            None,
        );
        assert_eq!(ids(&merged), [40, 30]);
        assert_eq!(merged.next, None);
    }

    #[test]
    fn a_post_in_both_lists_is_listed_once() {
        let merged = merge(
            FROM_THE_START,
            [answer(&[90, 70], 2, true), answer(&[90, 80], 2, true)],
            None,
        );
        assert_eq!(ids(&merged), [90, 80, 70]);
    }

    #[test]
    fn counts_posts_without_files_once_a_read_is_kept_whole() {
        // 3 answered, 1 of them without a file: the next read starts after all 3.
        let merged = merge(
            FROM_THE_START,
            [answer(&[90, 70], 3, false), answer(&[95], 1, true)],
            None,
        );
        assert_eq!(ids(&merged), [95, 90, 70]);
        assert_eq!(merged.next, places(Some(3), None));
    }

    #[test]
    fn a_post_out_of_order_is_read_again_rather_than_skipped() {
        // 60 comes before 75 in the photos' answer, though older: only the posts before it count as read.
        let merged = merge(
            FROM_THE_START,
            [
                answer(&[90, 60, 75, 70], 4, true),
                answer(&[80, 65], 2, false),
            ],
            None,
        );
        assert_eq!(ids(&merged), [90, 80, 75, 70, 65]);
        assert_eq!(merged.next, places(Some(1), Some(2)));
    }

    #[test]
    fn takes_the_account_from_either_list() {
        let account = AccountInfo {
            username: Some("NASA".to_owned()),
            ..AccountInfo::default()
        };
        let mut videos = answer(&[80], 1, true);
        if let Some(videos) = videos.as_mut() {
            videos.account = Some(account.clone());
        }
        let merged = merge(FROM_THE_START, [answer(&[], 0, true), videos], None);
        assert_eq!(merged.account, Some(account));
    }
}

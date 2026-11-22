//! An X account's Media tab, as X serves it since gallery-dl 1.32.20 follows it: two lists, the account's posts with
//! photos and its posts with videos, each newest first. gallery-dl reads the tab as one list after the other, every
//! photo post before the first video post, so the newest 50 of an account with many photos held no video, and a
//! watched account never met a new one. Each list is read on its own instead (`?filter=photos`, `?filter=videos`), both
//! at once, and the two are merged by date (`MediaReader`). Neither list holds animated GIFs: posts with only a GIF
//! can't be listed.
//!
//! **Places.** Each list is read from its own place (`ListPlace`): gallery-dl's cursor for a page of the list, which
//! it logs each time it moves on to the next (`gallery::last_cursor`), so a later read starts on the page where the
//! last one stopped, without asking for the pages before again. A read rarely stops at a page's end, so the next one
//! starts with the posts of that page handed over already: those at or above the oldest post handed over
//! (`MediaPlaces::handed_down_to`) are passed over. A list whose posts weren't all handed over (the other list hadn't
//! been read that far back) is read again from the same page, past the posts that were (`ListPlace::skip`).
//!
//! **X's limit.** X answers a signed-in account 50 requests for either list every 15 minutes, each list on its own
//! count (its `x-rate-limit-limit: 50` and `x-rate-limit-reset` headers, read on 2026-11-21: the 15 minutes start with
//! the first request). A request answers one page: 14 to 34 posts with photos on NASA (27 on average over 8 pages; 45
//! on IRURI the day before), always 20 posts with videos. gallery-dl stops asking with 1 to 5 requests left. Reads are
//! told to stop there (`gallery::STOP_AT_LIMIT`), answering what they read before; the next read of that list then
//! sits the wait out inside gallery-dl, which says until when, from X's own header (`gallery::waits_until`), and reads
//! on by itself.
//!
//! **Where a list ends.** Not always at the account's first post: NASA's photos' list ended after 470 posts, back to
//! April 2025 (X answered an empty page with its cursor unchanged, 2026-11-21), though X counts 28,260 photos and
//! videos there, while its videos' list was still going at 1,600 posts, back to February 2021. So X's count
//! (`Listing::media_total`) says at most how much there is to list.

use std::cmp::Reverse;
use std::collections::HashSet;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use super::{ListPlace, MediaPlaces};
use crate::cancel::CancelToken;
use crate::gallery::{self, AccountInfo, GalleryTools};
use crate::local_time;
use crate::metadata::{PreviewError, VideoInfo};
use crate::sync::lock;

/// How long X counts a list's requests before starting over.
pub(super) const WINDOW: Duration = Duration::from_secs(15 * 60);

/// About how many of an account's posts one such stretch lists before X's limit pauses it: 45 to 49 requests, of 20
/// posts each in the videos' list and 14 to 45 in the photos' (25 on average on NASA), less the page a read starts
/// again on. An account of photos alone or of videos alone lists about this many; one of both, up to twice as many.
/// In the app on 2026-11-21, NASA listed 1,210 posts before its first wait (its 470 with photos and 740 with videos),
/// then 860 with videos before the next.
pub(super) const POSTS_PER_WINDOW: u32 = 1_000;

/// How long one read may take: a few hundred posts, a second or less per page.
const READ_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// One of the Media tab's two lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MediaList {
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

    /// Its place among `BOTH`.
    fn index(self) -> usize {
        match self {
            Self::Photos => 0,
            Self::Videos => 1,
        }
    }
}

/// Where both lists start: neither read yet.
pub(super) const FROM_THE_START: MediaPlaces = MediaPlaces {
    photos: Some(ListPlace {
        cursor: None,
        skip: 0,
    }),
    videos: Some(ListPlace {
        cursor: None,
        skip: 0,
    }),
    handed_down_to: None,
};

/// What one read of a list answered.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ListAnswer {
    /// Its posts with files, in the list's order.
    pub posts: Vec<VideoInfo>,
    /// How many posts it answered, those without files included, as `--post-range` counts them.
    pub answered: u32,
    /// The list ended within this read.
    pub ended: bool,
    /// X's limit on the list's requests ran out before the posts asked for were read: `posts` are those read before.
    pub limited: bool,
    /// The page gallery-dl was on when it stopped (`gallery::last_cursor`), or `None` when it never left the page it
    /// started on.
    pub cursor: Option<String>,
    pub account: Option<AccountInfo>,
}

/// What reads a list: gallery-dl (`GalleryReader`), or a test's answers.
pub(super) trait ListReader: Sync {
    /// The list's posts from `place`, at most `count`. A read that `waits` sits X's limit out rather than stopping at
    /// it, telling `on_wait` until when once it waits (milliseconds since 1970), and `None` once it reads on.
    fn read(
        &self,
        list: MediaList,
        place: &ListPlace,
        count: u32,
        waits: bool,
        on_wait: &(dyn Fn(Option<i64>) + Sync),
    ) -> Result<ListAnswer, PreviewError>;
}

/// Reads an account's lists with gallery-dl.
pub(super) struct GalleryReader<'a> {
    pub images: &'a GalleryTools<'a>,
    /// The account's Media tab, as `listing_link` gives it.
    pub media_tab: &'a str,
    pub cancel: &'a CancelToken,
}

impl ListReader for GalleryReader<'_> {
    fn read(
        &self,
        list: MediaList,
        place: &ListPlace,
        count: u32,
        waits: bool,
        on_wait: &(dyn Fn(Option<i64>) + Sync),
    ) -> Result<ListAnswer, PreviewError> {
        let link = list.link(self.media_tab);
        let range = format!("{}-{}", place.skip + 1, place.skip.saturating_add(count));
        let cursor = place
            .cursor
            .as_deref()
            .map(|cursor| format!("cursor={cursor}"));
        // Its debug lines say which page it stopped on, for the next read to start there (`gallery::last_cursor`).
        let mut extra = vec!["--post-range", range.as_str(), "--verbose"];
        if let Some(cursor) = &cursor {
            extra.extend(["-o", cursor.as_str()]);
        }
        let timeout = if waits {
            READ_TIMEOUT + WINDOW
        } else {
            extra.extend(gallery::STOP_AT_LIMIT);
            READ_TIMEOUT
        };
        let mut waiting = false;
        let answer = gallery::run_watched(
            self.images,
            &link,
            &extra,
            timeout,
            self.cancel,
            &mut |line| match gallery::waits_until(line) {
                Some(time_of_day) => {
                    waiting = true;
                    on_wait(Some(local_time::next(time_of_day, WINDOW)));
                }
                // Its next line comes once it asks again.
                None if waiting => {
                    waiting = false;
                    on_wait(None);
                }
                None => {}
            },
        );
        if waiting {
            on_wait(None);
        }
        let answer = answer?;
        let (found, limited) = gallery::parse_posts_before_limit(&answer.json, &link)?;
        let answered = u32::try_from(found.answered).unwrap_or(u32::MAX);
        Ok(ListAnswer {
            posts: found.posts,
            answered,
            ended: !limited && answered < count,
            limited,
            cursor: gallery::last_cursor(&answer.log),
            account: found.account,
        })
    }
}

/// One list as `MediaReader` reads it.
#[derive(Debug)]
struct ListState {
    /// Where its next read starts, or `None` once it ended.
    next: Option<ListPlace>,
    /// Its posts read and not handed over yet, in the list's order, each with its place in the read that gave it:
    /// older than where the other list was read to.
    carried: Vec<(u32, VideoInfo)>,
    /// Where the read that gave `carried` started: a later run finds them again from there.
    carried_from: Option<ListPlace>,
    /// Its last read stopped at X's limit: the next one sits the wait out.
    limited: bool,
}

impl ListState {
    fn at(place: Option<ListPlace>) -> Self {
        Self {
            next: place,
            carried: Vec::new(),
            carried_from: None,
            limited: false,
        }
    }

    /// Takes what a read from `from` answered. Its first posts at or above `handed_down_to`, the oldest post handed
    /// over, were handed over before (the read started on a page the last one stopped in), so they're passed over:
    /// only the first ones, since a post out of the list's order further down was never met.
    fn take(&mut self, answer: ListAnswer, from: ListPlace, handed_down_to: Option<u64>) {
        self.next = if answer.ended {
            None
        } else {
            Some(match answer.cursor {
                Some(cursor) if from.cursor.as_ref() != Some(&cursor) => ListPlace {
                    cursor: Some(cursor),
                    skip: 0,
                },
                // Still on the page it started on: the next read passes over what this one answered.
                _ => ListPlace {
                    cursor: from.cursor.clone(),
                    skip: from.skip.saturating_add(answer.answered),
                },
            })
        };
        self.limited = answer.limited;
        self.carried = (0..)
            .zip(answer.posts)
            .skip_while(|(_, post)| handed_down_to.is_some_and(|oldest| order(post) >= oldest))
            .collect();
        self.carried_from = Some(from);
    }

    /// Where a later run reads the list from: the first of its posts not handed over yet, counted from where their
    /// read started, else where its next read starts.
    fn place(&self) -> Option<ListPlace> {
        match (self.carried.first(), &self.carried_from) {
            (Some((place, _)), Some(from)) => Some(ListPlace {
                cursor: from.cursor.clone(),
                skip: from.skip.saturating_add(*place),
            }),
            _ => self.next.clone(),
        }
    }
}

/// What one step of `MediaReader` hands over.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Step {
    /// The next posts of both lists, newest first, each post once: as many as the list that reached less far back
    /// gave, and the other's among them. None when a list stopped at X's limit before anything of the other could be
    /// placed.
    pub posts: Vec<VideoInfo>,
    /// Where a later run goes on, or `None` once both lists ended and every post read was handed over.
    pub next: Option<MediaPlaces>,
    /// A list stopped at X's limit: the next step waits for it.
    pub limited: bool,
}

/// Reads an account's two lists in step and hands their posts over merged, newest first. A step reads each list that
/// has nothing left to hand over, both at once, then hands over the posts down to the newest place where a list that
/// goes on was read to: the other list's older posts may come after that list's next ones, so they're kept for a later
/// step (`set_aside`) rather than read again. So the list read to the newest place always hands over all it read, and
/// its next read starts on a new page, while the other is read again only once all it gave is handed over.
pub(super) struct MediaReader<'a> {
    reader: &'a dyn ListReader,
    /// Photos, then videos.
    lists: [ListState; 2],
    /// The ID of the oldest post handed over, by this run or those before.
    handed_down_to: Option<u64>,
    account: Option<AccountInfo>,
}

impl<'a> MediaReader<'a> {
    pub(super) fn new(reader: &'a dyn ListReader, from: MediaPlaces) -> Self {
        Self {
            reader,
            handed_down_to: from
                .handed_down_to
                .as_deref()
                .and_then(|id| id.parse().ok()),
            lists: [ListState::at(from.photos), ListState::at(from.videos)],
            account: None,
        }
    }

    /// Reads at most `count` posts of each list that has none left to hand over, and hands over what both lists have
    /// down to where a list that goes on stopped. A list whose last read stopped at X's limit waits for it first:
    /// `on_pause` is told until when (milliseconds since 1970), and `None` once every list reads on.
    pub(super) fn step(
        &mut self,
        count: u32,
        on_pause: &(dyn Fn(Option<i64>) + Sync),
    ) -> Result<Step, PreviewError> {
        self.read_lists(count, on_pause)?;
        let posts = self.hand_over();
        Ok(Step {
            posts,
            next: self.places(),
            limited: self.lists.iter().any(|list| list.limited),
        })
    }

    /// The account, from whichever list gave it.
    pub(super) fn account(&self) -> Option<AccountInfo> {
        self.account.clone()
    }

    /// The posts read and not handed over yet, each once: what they are is known, their place isn't yet.
    pub(super) fn set_aside(&self) -> Vec<VideoInfo> {
        let mut met = HashSet::new();
        self.lists
            .iter()
            .flat_map(|list| &list.carried)
            .filter(|(_, post)| met.insert(post.video_id.as_str()))
            .map(|(_, post)| post.clone())
            .collect()
    }

    fn read_lists(
        &mut self,
        count: u32,
        on_pause: &(dyn Fn(Option<i64>) + Sync),
    ) -> Result<(), PreviewError> {
        let reader = self.reader;
        let pauses = Pauses {
            until: Mutex::new([None; 2]),
            on_pause,
        };
        let reads = self.lists.each_ref().map(|list| {
            list.carried
                .is_empty()
                .then(|| list.next.clone())
                .flatten()
                .map(|place| (place, list.limited))
        });
        let answers = thread::scope(|scope| {
            let running = MediaList::BOTH.map(|list| {
                reads[list.index()].as_ref().map(|(place, waits)| {
                    let pauses = &pauses;
                    scope.spawn(move || {
                        reader.read(list, place, count, *waits, &|until| pauses.set(list, until))
                    })
                })
            });
            running.map(|read| {
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
        for ((list, answer), read) in self.lists.iter_mut().zip(answers).zip(reads) {
            let (Some(mut answer), Some((place, _))) = (answer, read) else {
                continue;
            };
            if self.account.is_none() {
                self.account = answer.account.take();
            }
            list.take(answer, place, self.handed_down_to);
        }
        Ok(())
    }

    /// The posts both lists have down to the newest place where a list that goes on was read to, newest first, each
    /// once (as the photos' list gave it); the lists keep the rest.
    fn hand_over(&mut self) -> Vec<VideoInfo> {
        // A list that goes on with nothing read (X's limit stopped it at once): its next posts may be newer than
        // anything the other list has.
        let blocked = self
            .lists
            .iter()
            .any(|list| list.next.is_some() && list.carried.is_empty());
        if blocked {
            return Vec::new();
        }
        let down_to = self
            .lists
            .iter()
            .filter(|list| list.next.is_some())
            .filter_map(|list| list.carried.iter().map(|(_, post)| order(post)).min())
            .max()
            .unwrap_or(0);

        let mut listed = HashSet::new();
        let mut posts: Vec<&VideoInfo> = self
            .lists
            .iter()
            .flat_map(|list| &list.carried)
            .map(|(_, post)| post)
            .filter(|post| order(post) >= down_to && listed.insert(post.video_id.as_str()))
            .collect();
        posts.sort_by_key(|post| Reverse(order(post)));
        let posts: Vec<VideoInfo> = posts.into_iter().cloned().collect();
        if !posts.is_empty() {
            self.handed_down_to = Some(
                self.handed_down_to
                    .map_or(down_to, |oldest| oldest.min(down_to)),
            );
        }
        for list in &mut self.lists {
            list.carried.retain(|(_, post)| order(post) < down_to);
        }
        posts
    }

    fn places(&self) -> Option<MediaPlaces> {
        let [photos, videos] = self.lists.each_ref().map(ListState::place);
        (photos.is_some() || videos.is_some()).then(|| MediaPlaces {
            photos,
            videos,
            handed_down_to: self.handed_down_to.map(|id| id.to_string()),
        })
    }
}

/// Until when each list waits for X's limit: a step reads on once the last of them does.
struct Pauses<'a> {
    until: Mutex<[Option<i64>; 2]>,
    on_pause: &'a (dyn Fn(Option<i64>) + Sync),
}

impl Pauses<'_> {
    fn set(&self, list: MediaList, until: Option<i64>) {
        let mut waits = lock(&self.until);
        waits[list.index()] = until;
        // Told under the lock, so two lists' news arrive in the order they came.
        (self.on_pause)(waits.iter().flatten().copied().max());
    }
}

/// A post's place in time: its ID, which X gives in the order posts are made. One without a numeric ID goes last.
fn order(post: &VideoInfo) -> u64 {
    post.video_id.parse().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

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
            tags: Vec::new(),
        }
    }

    /// An answer of these posts, of `answered` in all, which ended or not, without leaving its page.
    fn answer(ids: &[u64], answered: u32, ended: bool) -> ListAnswer {
        ListAnswer {
            posts: ids.iter().copied().map(post).collect(),
            answered,
            ended,
            limited: false,
            cursor: None,
            account: None,
        }
    }

    /// An answer that stopped on the page `cursor` names.
    fn on_page(ids: &[u64], cursor: &str) -> ListAnswer {
        ListAnswer {
            cursor: Some(cursor.to_owned()),
            ..answer(ids, ids.len() as u32, false)
        }
    }

    /// An answer cut short by X's limit.
    fn limited(ids: &[u64], cursor: Option<&str>) -> ListAnswer {
        ListAnswer {
            limited: true,
            cursor: cursor.map(str::to_owned),
            ..answer(ids, ids.len() as u32, false)
        }
    }

    /// One read asked of the lists.
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Asked {
        list: MediaList,
        place: ListPlace,
        count: u32,
        waits: bool,
    }

    /// Answers each list's reads in turn, and remembers what was asked. A read that `waits` says it waits until 5,000
    /// (photos) or 7,000 (videos), then that it reads on.
    #[derive(Default)]
    struct Answers {
        photos: Mutex<VecDeque<ListAnswer>>,
        videos: Mutex<VecDeque<ListAnswer>>,
        asked: Mutex<Vec<Asked>>,
    }

    impl Answers {
        fn of(photos: Vec<ListAnswer>, videos: Vec<ListAnswer>) -> Self {
            Self {
                photos: Mutex::new(photos.into()),
                videos: Mutex::new(videos.into()),
                asked: Mutex::default(),
            }
        }

        /// What was asked of one list, in order.
        fn asked(&self, list: MediaList) -> Vec<Asked> {
            lock(&self.asked)
                .iter()
                .filter(|asked| asked.list == list)
                .cloned()
                .collect()
        }
    }

    impl ListReader for Answers {
        fn read(
            &self,
            list: MediaList,
            place: &ListPlace,
            count: u32,
            waits: bool,
            on_wait: &(dyn Fn(Option<i64>) + Sync),
        ) -> Result<ListAnswer, PreviewError> {
            lock(&self.asked).push(Asked {
                list,
                place: place.clone(),
                count,
                waits,
            });
            if waits {
                on_wait(Some(match list {
                    MediaList::Photos => 5_000,
                    MediaList::Videos => 7_000,
                }));
                on_wait(None);
            }
            let answers = match list {
                MediaList::Photos => &self.photos,
                MediaList::Videos => &self.videos,
            };
            lock(answers)
                .pop_front()
                .ok_or_else(|| PreviewError::Failed {
                    detail: format!("{list:?} asked once too often"),
                })
        }
    }

    fn ids(posts: &[VideoInfo]) -> Vec<u64> {
        posts.iter().map(order).collect()
    }

    fn at(cursor: Option<&str>, skip: u32) -> ListPlace {
        ListPlace {
            cursor: cursor.map(str::to_owned),
            skip,
        }
    }

    fn places(
        photos: Option<ListPlace>,
        videos: Option<ListPlace>,
        handed_down_to: Option<u64>,
    ) -> Option<MediaPlaces> {
        Some(MediaPlaces {
            photos,
            videos,
            handed_down_to: handed_down_to.map(|id| id.to_string()),
        })
    }

    fn no_pause() -> impl Fn(Option<i64>) + Sync {
        |_| {}
    }

    /// One step from the start.
    fn first_step(answers: &Answers) -> (Step, Vec<u64>) {
        let mut reader = MediaReader::new(answers, FROM_THE_START);
        let step = reader.step(50, &no_pause()).unwrap();
        let set_aside = ids(&reader.set_aside());
        (step, set_aside)
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
        let answers = Answers::of(
            vec![answer(&[90, 70, 50], 3, true)],
            vec![answer(&[80, 60], 2, true)],
        );
        let (step, set_aside) = first_step(&answers);
        assert_eq!(ids(&step.posts), [90, 80, 70, 60, 50]);
        assert_eq!(step.next, None, "nothing more");
        assert!(set_aside.is_empty());
        assert!(!step.limited);
    }

    #[test]
    fn stops_where_a_list_that_goes_on_stopped() {
        // The photos go on past 70: a video older than that may have photos between it and 70 still unread.
        let answers = Answers::of(
            vec![on_page(&[90, 80, 70], "p2")],
            vec![answer(&[85, 40, 10], 3, true)],
        );
        let (step, set_aside) = first_step(&answers);
        assert_eq!(ids(&step.posts), [90, 85, 80, 70]);
        assert_eq!(
            step.next,
            places(Some(at(Some("p2"), 0)), Some(at(None, 1)), Some(70)),
            "the photos from the page they stopped on; the videos again, past the one handed over"
        );
        assert_eq!(set_aside, [40, 10], "read, but without a place yet");

        // Both go on: down to the newer of the two places where they stopped.
        let answers = Answers::of(
            vec![on_page(&[90, 80, 70], "p2")],
            vec![on_page(&[85, 75, 60], "v2")],
        );
        let (step, set_aside) = first_step(&answers);
        assert_eq!(ids(&step.posts), [90, 85, 80, 75, 70]);
        assert_eq!(
            step.next,
            places(Some(at(Some("p2"), 0)), Some(at(None, 2)), Some(70))
        );
        assert_eq!(set_aside, [60]);
    }

    #[test]
    fn keeps_what_it_read_for_the_next_step_rather_than_reading_it_again() {
        let answers = Answers::of(
            vec![on_page(&[90, 80, 70], "p2"), answer(&[65, 30], 2, true)],
            vec![answer(&[85, 40, 10], 3, true)],
        );
        let mut reader = MediaReader::new(&answers, FROM_THE_START);
        let first = reader.step(3, &no_pause()).unwrap();
        assert_eq!(ids(&first.posts), [90, 85, 80, 70]);

        // The videos have 40 and 10 left, so only the photos are read; they end, and everything is handed over.
        let second = reader.step(3, &no_pause()).unwrap();
        assert_eq!(ids(&second.posts), [65, 40, 30, 10]);
        assert_eq!(second.next, None);
        assert_eq!(
            answers.asked(MediaList::Videos).len(),
            1,
            "the videos were read once"
        );
        assert_eq!(
            answers.asked(MediaList::Photos)[1],
            Asked {
                list: MediaList::Photos,
                place: at(Some("p2"), 0),
                count: 3,
                waits: false,
            }
        );
    }

    #[test]
    fn the_list_read_less_far_back_hands_over_all_it_read() {
        // 3 asked of each: the photos reach back to 70 only, so all 3 are handed over, with the videos among them.
        let answers = Answers::of(
            vec![on_page(&[90, 80, 70], "p2")],
            vec![on_page(&[85, 60, 50], "v2")],
        );
        let mut reader = MediaReader::new(&answers, FROM_THE_START);
        let step = reader.step(3, &no_pause()).unwrap();
        assert_eq!(
            ids(&step.posts),
            [90, 85, 80, 70],
            "at least the 3 asked for"
        );
    }

    #[test]
    fn a_list_read_again_starts_at_its_first_post_not_handed_over() {
        // The run before left the photos on the page "p2", 80 and 70 of it handed over; this read's 60 is handed over
        // too, and 50 waits for the videos, which stopped at 55.
        let from = MediaPlaces {
            photos: Some(at(Some("p2"), 0)),
            videos: Some(at(Some("v2"), 0)),
            handed_down_to: Some("70".to_owned()),
        };
        let answers = Answers::of(
            vec![answer(&[80, 70, 60, 50], 4, true)],
            vec![on_page(&[65, 55], "v3")],
        );
        let mut reader = MediaReader::new(&answers, from);
        let step = reader.step(4, &no_pause()).unwrap();
        assert_eq!(ids(&step.posts), [65, 60, 55]);
        assert_eq!(
            step.next,
            places(Some(at(Some("p2"), 3)), Some(at(Some("v3"), 0)), Some(55)),
            "the photos again from their page, past the 3 posts handed over"
        );
    }

    #[test]
    fn a_later_run_passes_over_the_posts_handed_over_before() {
        // The run before handed over down to 70, and the photos stopped on the page that starts with 80.
        let from = MediaPlaces {
            photos: Some(at(Some("p2"), 0)),
            videos: None,
            handed_down_to: Some("70".to_owned()),
        };
        let answers = Answers::of(vec![on_page(&[80, 70, 60, 50], "p3")], vec![]);
        let mut reader = MediaReader::new(&answers, from);
        let step = reader.step(4, &no_pause()).unwrap();
        assert_eq!(ids(&step.posts), [60, 50], "80 and 70 were listed before");
        assert_eq!(
            step.next,
            places(Some(at(Some("p3"), 0)), None, Some(50)),
            "the videos ended before, and aren't read again"
        );
        assert!(answers.asked(MediaList::Videos).is_empty());
    }

    #[test]
    fn a_read_that_stays_on_its_page_passes_over_what_it_answered() {
        // 19 asked, as a check asks: fewer than X's first page holds, so gallery-dl names no next page.
        let answers = Answers::of(
            vec![answer(&[90, 80], 19, false), answer(&[70], 1, true)],
            vec![answer(&[], 0, true)],
        );
        let mut reader = MediaReader::new(&answers, FROM_THE_START);
        let first = reader.step(19, &no_pause()).unwrap();
        assert_eq!(ids(&first.posts), [90, 80]);
        assert_eq!(first.next, places(Some(at(None, 19)), None, Some(80)));
        reader.step(19, &no_pause()).unwrap();
        assert_eq!(answers.asked(MediaList::Photos)[1].place, at(None, 19));
    }

    #[test]
    fn a_post_in_both_lists_is_listed_once() {
        let answers = Answers::of(
            vec![answer(&[90, 70], 2, true)],
            vec![answer(&[90, 80], 2, true)],
        );
        let (step, set_aside) = first_step(&answers);
        assert_eq!(ids(&step.posts), [90, 80, 70]);
        assert!(set_aside.is_empty());
    }

    #[test]
    fn a_post_out_of_order_is_kept_rather_than_skipped() {
        // 60 comes before 75 in the photos' answer, though older: it waits for the videos to reach it.
        let answers = Answers::of(
            vec![answer(&[90, 60, 75, 70], 4, true)],
            vec![on_page(&[80, 65], "v2"), answer(&[55], 1, true)],
        );
        let mut reader = MediaReader::new(&answers, FROM_THE_START);
        let first = reader.step(4, &no_pause()).unwrap();
        assert_eq!(ids(&first.posts), [90, 80, 75, 70, 65]);
        assert_eq!(ids(&reader.set_aside()), [60]);
        let second = reader.step(4, &no_pause()).unwrap();
        assert_eq!(ids(&second.posts), [60, 55]);
        assert_eq!(second.next, None);
    }

    #[test]
    fn hands_over_what_was_read_before_xs_limit_and_waits_at_the_next_step() {
        // The photos stop at the limit after two pages; the videos ended.
        let answers = Answers::of(
            vec![limited(&[90, 80, 70], Some("p3")), on_page(&[60, 50], "p5")],
            vec![answer(&[85, 40], 2, true)],
        );
        let pauses = Mutex::new(Vec::new());
        let on_pause = |until: Option<i64>| lock(&pauses).push(until);
        let mut reader = MediaReader::new(&answers, FROM_THE_START);

        let first = reader.step(400, &on_pause).unwrap();
        assert_eq!(ids(&first.posts), [90, 85, 80, 70]);
        assert!(first.limited);
        assert_eq!(
            first.next,
            places(Some(at(Some("p3"), 0)), Some(at(None, 1)), Some(70)),
            "the photos from the page X refused"
        );
        assert!(lock(&pauses).is_empty(), "nothing waits yet");

        let second = reader.step(400, &on_pause).unwrap();
        assert_eq!(ids(&second.posts), [60, 50]);
        assert!(!second.limited);
        let asked = answers.asked(MediaList::Photos);
        assert!(!asked[0].waits);
        assert_eq!(
            (asked[1].waits, &asked[1].place),
            (true, &at(Some("p3"), 0)),
            "the read after the limit sits the wait out, from the page X refused"
        );
        assert_eq!(*lock(&pauses), [Some(5_000), None]);
    }

    #[test]
    fn hands_nothing_over_while_a_list_stopped_at_the_limit_at_once() {
        // The photos' first request is refused: where they'd come among the videos isn't known.
        let answers = Answers::of(
            vec![limited(&[], None), answer(&[90, 70], 2, true)],
            vec![answer(&[80, 60], 2, true)],
        );
        let mut reader = MediaReader::new(&answers, FROM_THE_START);
        let first = reader.step(50, &no_pause()).unwrap();
        assert!(first.posts.is_empty());
        assert!(first.limited);
        assert_eq!(
            first.next,
            places(Some(at(None, 0)), Some(at(None, 0)), None)
        );
        let second = reader.step(50, &no_pause()).unwrap();
        assert_eq!(ids(&second.posts), [90, 80, 70, 60]);
        assert_eq!(
            answers.asked(MediaList::Videos).len(),
            1,
            "the videos read weren't asked for again"
        );
    }

    #[test]
    fn says_until_when_the_last_list_waits() {
        let answers = Answers::of(
            vec![limited(&[], None), answer(&[90], 1, true)],
            vec![limited(&[], None), answer(&[80], 1, true)],
        );
        let pauses = Mutex::new(Vec::new());
        let on_pause = |until: Option<i64>| lock(&pauses).push(until);
        let mut reader = MediaReader::new(&answers, FROM_THE_START);
        reader.step(50, &on_pause).unwrap();
        let step = reader.step(50, &on_pause).unwrap();
        assert_eq!(ids(&step.posts), [90, 80]);
        let told = lock(&pauses).clone();
        assert_eq!(told.last(), Some(&None), "both read on in the end");
        assert!(told.contains(&Some(7_000)), "{told:?}");
        assert!(
            told.iter()
                .flatten()
                .all(|until| [5_000, 7_000].contains(until)),
            "{told:?}"
        );
    }

    #[test]
    fn takes_the_account_from_either_list() {
        let account = AccountInfo {
            username: Some("NASA".to_owned()),
            ..AccountInfo::default()
        };
        let mut videos = answer(&[80], 1, true);
        videos.account = Some(account.clone());
        let answers = Answers::of(vec![answer(&[], 0, true)], vec![videos]);
        let mut reader = MediaReader::new(&answers, FROM_THE_START);
        reader.step(50, &no_pause()).unwrap();
        assert_eq!(reader.account(), Some(account));
    }

    #[test]
    fn a_failed_read_fails_the_step() {
        let answers = Answers::of(vec![answer(&[90], 1, true)], vec![]);
        let mut reader = MediaReader::new(&answers, FROM_THE_START);
        assert!(matches!(
            reader.step(50, &no_pause()),
            Err(PreviewError::Failed { .. })
        ));
    }
}

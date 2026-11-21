//! The local clock, for times a tool gives as a time of day: gallery-dl says until when it waits for X's limit as
//! "01:12:17" (`gallery::waits_until`).

use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DAY: u32 = 24 * 60 * 60;

/// When the local clock next shows `time_of_day` (seconds since midnight), in milliseconds since 1970. A time further
/// off than `at_most` isn't believed (a clock set back for winter between the two, or no local clock to read): it's
/// `at_most` from now instead.
pub(crate) fn next(time_of_day: u32, at_most: Duration) -> i64 {
    let ahead = seconds_of_day()
        .map(|now| seconds_ahead(now, time_of_day))
        .filter(|ahead| u64::from(*ahead) <= at_most.as_secs())
        .map_or(at_most, |ahead| Duration::from_secs(u64::from(ahead)));
    let then = SystemTime::now() + ahead;
    then.duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|since| i64::try_from(since.as_millis()).ok())
        .unwrap_or(i64::MAX)
}

/// How many seconds from `now` until the clock shows `then`, both in seconds since midnight: later today, or tomorrow
/// when `then` is past.
fn seconds_ahead(now: u32, then: u32) -> u32 {
    (then % DAY + DAY - now % DAY) % DAY
}

/// The local time of day, in seconds since midnight.
#[cfg(windows)]
fn seconds_of_day() -> Option<u32> {
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;

    // SAFETY: GetLocalTime only writes the SYSTEMTIME it's given, which lives through the call.
    let time = unsafe {
        let mut time: SYSTEMTIME = std::mem::zeroed();
        GetLocalTime(&mut time);
        time
    };
    Some(u32::from(time.wHour) * 3600 + u32::from(time.wMinute) * 60 + u32::from(time.wSecond))
}

/// Tsundoku only runs on Windows; elsewhere, where only its tests build, the local time isn't read.
#[cfg(not(windows))]
fn seconds_of_day() -> Option<u32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_the_seconds_until_a_time_of_day() {
        assert_eq!(seconds_ahead(3600, 3600 + 14 * 60), 14 * 60);
        assert_eq!(seconds_ahead(3600, 3600), 0);
        // Ten minutes to midnight, until five past.
        assert_eq!(seconds_ahead(DAY - 600, 300), 900);
    }

    #[test]
    fn a_time_too_far_off_is_taken_as_the_longest_wait() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_millis() as i64);
        let quarter = Duration::from_secs(15 * 60);
        // Whatever the clock shows, a time 12 hours off is more than a quarter of an hour away.
        let far = seconds_of_day().map_or(0, |now| (now + DAY / 2) % DAY);
        let until = next(far, quarter);
        assert!((until - now - 900_000).abs() < 5_000, "{}", until - now);
        if let Some(now_of_day) = seconds_of_day() {
            let soon = next((now_of_day + 120) % DAY, quarter);
            assert!((soon - now - 120_000).abs() < 5_000, "{}", soon - now);
        }
    }
}

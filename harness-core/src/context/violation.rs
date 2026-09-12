//! Typed layout violations and the volatile-data detectors (plan todo 11).
//!
//! The layout law: no volatile data may enter any prompt segment above L2.
//! The detectors below recognize the shapes a harness actually injects —
//! ISO-8601 datetimes, unix-epoch `timestamp` fields, and wall-clock tags —
//! and the builder turns a hit into a typed [`LayoutViolation`]
//! (which segment, at which byte offset, with evidence) instead of
//! silently shipping a prefix-breaking prompt.

use std::fmt;

use super::layout::SegmentName;

/// The volatile pattern taxonomy detected above L2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolatileKind {
    /// An ISO-8601 datetime (`YYYY-MM-DDThh:mm` or with a space).
    IsoDateTime,
    /// A unix-epoch timestamp field (`timestamp: 1712345678`).
    UnixTimestamp,
    /// A wall-clock tag (`<current_time>` / `<timestamp>`).
    TimeTag,
}

/// A rejected injection of volatile data into a stable segment.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub struct LayoutViolation {
    /// The stable segment the volatile data was found in.
    pub segment: SegmentName,
    /// Byte offset of the pattern within that segment's content.
    pub byte_offset: usize,
    /// Which volatile shape matched.
    pub kind: VolatileKind,
    /// The matched bytes (evidence for the rejection).
    pub evidence: String,
}

impl fmt::Display for LayoutViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "volatile {:?} in segment {:?} at byte {}: {}",
            self.kind, self.segment, self.byte_offset, self.evidence
        )
    }
}

/// Why the builder refused to compose a prompt.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BuildError {
    /// Volatile data above L2 (the layout law).
    #[error("volatile data above L2 rejected: {0}")]
    Layout(#[from] LayoutViolation),
    /// The volatile tail exceeded its byte bound.
    #[error("volatile tail of {len} bytes exceeds the bound of {max}")]
    OversizedTail {
        /// Actual tail length.
        len: usize,
        /// The maximum allowed.
        max: usize,
    },
}

/// Scan `content` for a volatile pattern. Returns the kind, the byte
/// offset, and the matched evidence of the earliest hit.
///
/// The detectors are deliberately conservative: a bare date (`2026-09-12`)
/// is NOT volatile (documents legitimately contain dates); the datetime
/// detector requires a time component, the unix detector requires the
/// `timestamp` keyword plus a 10-13 digit run, and the tag detector
/// matches only the two known wall-clock tag spellings.
pub(crate) fn scan_volatile(content: &str) -> Option<(VolatileKind, usize, String)> {
    let hits = [
        find_time_tag(content).map(|(at, ev)| (VolatileKind::TimeTag, at, ev)),
        find_unix_timestamp(content).map(|(at, ev)| (VolatileKind::UnixTimestamp, at, ev)),
        find_iso_datetime(content).map(|(at, ev)| (VolatileKind::IsoDateTime, at, ev)),
    ];
    hits.into_iter().flatten().min_by_key(|(_, at, _)| *at)
}

fn find_time_tag(content: &str) -> Option<(usize, String)> {
    for tag in ["<current_time", "<timestamp"] {
        if let Some(at) = content.find(tag) {
            return Some((at, tag.to_owned()));
        }
    }
    None
}

fn find_unix_timestamp(content: &str) -> Option<(usize, String)> {
    let bytes = content.as_bytes();
    let keyword = b"timestamp";
    let mut index = 0;
    while index + keyword.len() <= bytes.len() {
        if bytes[index..index + keyword.len()].eq_ignore_ascii_case(keyword) {
            let mut cursor = index + keyword.len();
            let mut separators = 0;
            while separators < 4
                && cursor < bytes.len()
                && matches!(bytes[cursor], b' ' | b':' | b'=' | b'"' | b'\'')
            {
                cursor += 1;
                separators += 1;
            }
            let digits_start = cursor;
            while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
                cursor += 1;
            }
            let digit_count = cursor - digits_start;
            if (10..=13).contains(&digit_count) {
                return Some((index, content[index..cursor].to_owned()));
            }
            index += 1;
        } else {
            index += 1;
        }
    }
    None
}

fn find_iso_datetime(content: &str) -> Option<(usize, String)> {
    let bytes = content.as_bytes();
    const LEN: usize = 16;
    if bytes.len() < LEN {
        return None;
    }
    for index in 0..=bytes.len() - LEN {
        if is_iso_datetime_at(bytes, index) {
            return Some((index, content[index..index + LEN].to_owned()));
        }
    }
    None
}

fn is_iso_datetime_at(bytes: &[u8], at: usize) -> bool {
    let digits = |range: std::ops::Range<usize>| {
        bytes
            .get(range)
            .is_some_and(|slice| slice.iter().all(u8::is_ascii_digit))
    };
    let two = |offset: usize| {
        let pair = bytes.get(at + offset..at + offset + 2)?;
        let value = (pair[0] - b'0') * 10 + (pair[1] - b'0');
        Some(value)
    };
    digits(at..at + 4)
        && bytes.get(at + 4) == Some(&b'-')
        && digits(at + 5..at + 7)
        && bytes.get(at + 7) == Some(&b'-')
        && digits(at + 8..at + 10)
        && matches!(bytes.get(at + 10), Some(b'T') | Some(b't') | Some(b' '))
        && digits(at + 11..at + 13)
        && bytes.get(at + 13) == Some(&b':')
        && digits(at + 14..at + 16)
        && matches!(two(5), Some(1..=12))
        && matches!(two(8), Some(1..=31))
        && matches!(two(11), Some(0..=23))
        && matches!(two(14), Some(0..=59))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_rejects_known_injection_shapes_at_their_offsets() {
        let (kind, at, _) = scan_volatile("wait for 2026-09-12T10:30:00Z").expect("iso hit");
        assert_eq!(kind, VolatileKind::IsoDateTime);
        assert_eq!(at, 9);
        let (kind, at, _) = scan_volatile("state\ntimestamp=1712345678999\nend").expect("unix hit");
        assert_eq!(kind, VolatileKind::UnixTimestamp);
        assert_eq!(at, 6);
        let (kind, at, _) = scan_volatile("clock <current_time>x</current_time>").expect("tag");
        assert_eq!(kind, VolatileKind::TimeTag);
        assert_eq!(at, 6);
    }

    #[test]
    fn scan_accepts_plain_dates_and_short_digit_runs() {
        // Bare dates, version numbers, and short runs are not volatile.
        assert_eq!(scan_volatile("released 2026-09-12 and 1.2.3.4"), None);
        assert_eq!(scan_volatile("count: 12345 for timestamp field"), None);
        assert_eq!(scan_volatile("normal text with 10:30 times"), None);
    }
}

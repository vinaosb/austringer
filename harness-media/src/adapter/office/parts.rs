//! Shared OOXML package-reading helpers (M18): zip part access under
//! per-part caps, the budget ledger every walk loop honors, and the
//! quick-xml event/attribute/relationship utilities common to the
//! docx/xlsx/pptx extractors.

use std::io::Read;
use std::time::Instant;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::error::MediaError;
use crate::extract::ExtractBudgets;

/// One OOXML part may not exceed this when materialized (a
/// pathological part trips the typed [`MediaError::ExtractTooLarge`],
/// never an unbounded read).
const MAX_PART_BYTES: u64 = 8 * 1024 * 1024;

/// Bound on relationship entries one package may carry.
const MAX_RELS: usize = 1024;

/// Budget ledger carried through every walk loop: the wall-clock
/// deadline and the cumulative materialized-byte cap, both from the
/// caller's [`ExtractBudgets`]. Trips are always the typed
/// [`MediaError::ExtractTooLarge`].
pub(super) struct Ledger {
    started: Instant,
    collected: u64,
    budgets: ExtractBudgets,
}

impl Ledger {
    /// Starts the ledger by charging the whole input (already
    /// materialized by the caller); an over-cap input refuses before
    /// the first part read.
    pub(super) fn new(bytes_len: usize, budgets: ExtractBudgets) -> Result<Self, MediaError> {
        let ledger = Self {
            started: Instant::now(),
            collected: bytes_len as u64,
            budgets,
        };
        ledger.check()?;
        Ok(ledger)
    }

    /// Charges `bytes` more materialized output against the cap.
    pub(super) fn charge(&mut self, bytes: usize) -> Result<(), MediaError> {
        self.collected = self.collected.saturating_add(bytes as u64);
        self.check()
    }

    /// The deadline / size-cap gate (called per parsed event).
    pub(super) fn check(&self) -> Result<(), MediaError> {
        if self.started.elapsed() >= self.budgets.deadline
            || self.collected > self.budgets.max_total_bytes
        {
            return Err(MediaError::ExtractTooLarge);
        }
        Ok(())
    }
}

/// An opened OOXML package over in-memory bytes.
pub(super) struct Package<'a> {
    archive: zip::ZipArchive<std::io::Cursor<&'a [u8]>>,
}

impl<'a> Package<'a> {
    /// Opens the zip central directory; non-zip bytes are the typed
    /// [`MediaError::ParseFailure`].
    pub(super) fn open(bytes: &'a [u8]) -> Result<Self, MediaError> {
        let archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
            .map_err(|_| MediaError::ParseFailure)?;
        Ok(Self { archive })
    }

    /// Whether a part with this exact name exists (the vbaProject.bin
    /// macro disclosure).
    pub(super) fn has_part(&self, name: &str) -> bool {
        self.archive.file_names().any(|part| part == name)
    }

    /// Reads one part under the per-part cap; a missing part or a read
    /// failure is the typed [`MediaError::ParseFailure`] (the caller
    /// dispatched on the probed format, so a missing document part
    /// means the package is not that format).
    pub(super) fn read_part(&mut self, name: &str) -> Result<Vec<u8>, MediaError> {
        let entry = self
            .archive
            .by_name(name)
            .map_err(|_| MediaError::ParseFailure)?;
        let mut buffer = Vec::new();
        entry
            .take(MAX_PART_BYTES + 1)
            .read_to_end(&mut buffer)
            .map_err(|_| MediaError::ParseFailure)?;
        if buffer.len() as u64 > MAX_PART_BYTES {
            return Err(MediaError::ExtractTooLarge);
        }
        Ok(buffer)
    }

    /// Reads one required utf-8 XML part.
    pub(super) fn read_xml(&mut self, name: &str) -> Result<String, MediaError> {
        let bytes = self.read_part(name)?;
        String::from_utf8(bytes).map_err(|_| MediaError::ParseFailure)
    }

    /// Reads one optional utf-8 XML part (absent -> `None`).
    pub(super) fn read_xml_optional(&mut self, name: &str) -> Result<Option<String>, MediaError> {
        if self.has_part(name) {
            self.read_xml(name).map(Some)
        } else {
            Ok(None)
        }
    }
}

/// Reads the next event under the ledger; malformed XML is the typed
/// [`MediaError::ParseFailure`].
pub(super) fn next_event<'a>(
    reader: &mut Reader<&'a [u8]>,
    ledger: &mut Ledger,
) -> Result<Event<'a>, MediaError> {
    ledger.check()?;
    reader.read_event().map_err(|_| MediaError::ParseFailure)
}

/// One general entity reference resolved to its character: the five
/// predefined XML names and numeric references only - anything else is
/// `None` (the caller refuses typed; custom entities never expand).
pub(super) fn reference_char(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "apos" => Some('\''),
        "quot" => Some('"'),
        _ => name
            .strip_prefix("#x")
            .and_then(|digits| u32::from_str_radix(digits, 16).ok())
            .or_else(|| {
                name.strip_prefix('#')
                    .and_then(|digits| digits.parse::<u32>().ok())
            })
            .and_then(char::from_u32),
    }
}

/// The local (namespace-stripped) attribute value for `name`, if the
/// element carries it.
pub(super) fn attribute(element: &BytesStart<'_>, name: &str) -> Option<String> {
    element.attributes().flatten().find_map(|attribute| {
        (attribute.key.local_name().as_ref() == name)
            .then(|| {
                attribute
                    .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                    .ok()
                    .map(|value| value.into_owned())
            })
            .flatten()
    })
}

/// The value of a PREFIXED attribute matched by its full raw key
/// (`r:id`, `r:embed`): office parts universally bind the office
/// relationships prefix to `r:`, and matching the prefixed form keeps
/// `id` (numeric) and `r:id` (relationship) distinguishable.
pub(super) fn prefixed_attribute(element: &BytesStart<'_>, key: &str) -> Option<String> {
    element.attributes().flatten().find_map(|attribute| {
        (attribute.key.as_ref() == key)
            .then(|| {
                attribute
                    .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                    .ok()
                    .map(|value| value.into_owned())
            })
            .flatten()
    })
}

/// One package relationship entry.
pub(super) struct Rel {
    /// The relationship id (`rId5`).
    pub id: String,
    /// The target (part name or external URI).
    pub target: String,
    /// Whether TargetMode="External".
    pub external: bool,
}

/// Parses a `.rels` part into its Relationship entries.
pub(super) fn parse_rels(xml: &str, ledger: &mut Ledger) -> Result<Vec<Rel>, MediaError> {
    let mut reader = Reader::from_str(xml);
    let mut rels = Vec::new();
    loop {
        match next_event(&mut reader, ledger)? {
            Event::Eof => break,
            Event::Start(element) | Event::Empty(element)
                if element.name().local_name().as_ref() == "Relationship" =>
            {
                if rels.len() >= MAX_RELS {
                    return Err(MediaError::ExtractTooLarge);
                }
                rels.push(Rel {
                    id: attribute(&element, "Id").unwrap_or_default(),
                    target: attribute(&element, "Target").unwrap_or_default(),
                    external: attribute(&element, "TargetMode").as_deref() == Some("External"),
                });
            }
            _ => {}
        }
    }
    Ok(rels)
}

/// Truncate to at most `max` CHARS, at a char boundary.
pub(super) fn truncate_chars(raw: String, max: usize) -> String {
    match raw.char_indices().nth(max) {
        Some((index, _)) => raw[..index].to_owned(),
        None => raw,
    }
}

/// Whitespace-collapse: runs of whitespace become one space, ends
/// trimmed.
pub(super) fn collapse(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

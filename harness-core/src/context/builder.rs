//! The prefix-cache-stable prompt builder (plan todo 11).
//!
//! [`PromptBuilder::render`] composes a [`ContextSpec`] into the fixed
//! seven-segment layout. The composition is a pure function of the spec:
//! slot segments are normalized (sorted by id, JSON contents re-serialized
//! with sorted keys), the frozen harness-composed inputs are scanned for
//! volatile data (a hit is a typed [`LayoutViolation`], never a shipped
//! prefix-churn), the tail is bounded, and history renders in sequence
//! order. Same logical state, same bytes — the layout law.

use super::effort::EffortSettings;
use super::history::render_history;
use super::layout::{SegmentMap, SegmentName, SegmentSpan, Stability};
use super::violation::{BuildError, LayoutViolation, scan_volatile};
use crate::event::{ContentHash, RecordedEvent};

/// One typed byte-stable segment feeding the `[L0,L1)` rules/skills
/// slots. Later todos supply real content; the builder normalizes
/// whatever arrives so rogue ordering cannot churn the prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotSegment {
    /// Stable identity; slots sort by `(id, content)` regardless of
    /// insertion order.
    pub id: String,
    /// The segment body. Valid-JSON bodies re-serialize with sorted keys.
    pub content: String,
}

/// Everything one prompt is composed from. History must carry only the
/// events preceding the volatile tail; the tail is the last user message
/// alone.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextSpec {
    /// System prompt text (frozen per epoch).
    pub system: String,
    /// The T6 base tool-schema render (frozen per epoch).
    pub base_schema: String,
    /// Rules slot segments (frozen per epoch; e.g. the T5 focus chain
    /// rendered at epoch start).
    pub rules: Vec<SlotSegment>,
    /// Skills slot segments (frozen per epoch).
    pub skills: Vec<SlotSegment>,
    /// The repository map (frozen per epoch; durable updates go
    /// in-history, never here).
    pub repomap: String,
    /// Append-only LLM-visible history preceding the tail.
    pub history: Vec<RecordedEvent>,
    /// The volatile tail: the ONLY mutable region.
    pub last_user_message: String,
    /// Effort settings rendered into the system segment (epoch-pinned).
    pub effort: EffortSettings,
    /// Per-message beta-channel effort: request metadata the adapter
    /// fills; never rendered into prompt bytes.
    pub beta_effort: Option<EffortSettings>,
}

/// A rendered prompt plus its typed segment map and cache metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderedPrompt {
    /// The canonical prompt bytes (the full seven-segment layout).
    pub prompt: String,
    /// The segment boundary record adapters consume.
    pub map: SegmentMap,
    /// Blake3 hash of the stable prefix (the `[0,L2)` bytes).
    pub stable_prefix_hash: ContentHash,
    /// The beta-channel effort pass-through, when requested.
    pub beta_effort: Option<EffortSettings>,
}

impl RenderedPrompt {
    /// The stable-prefix length in bytes (everything above the tail).
    #[must_use]
    pub const fn stable_prefix_bytes(&self) -> usize {
        self.map.l2()
    }

    /// Bytes of this prompt the NEXT turn's layout is guaranteed to
    /// preserve: the stable content through the end of history content.
    /// The closing history fence (`\n</history>\n`) is necessarily
    /// rewritten once history grows, so the provider's realistic maximum
    /// cache hit for the next turn is this offset, not L2.
    #[must_use]
    pub fn cacheable_prefix_bytes(&self) -> usize {
        self.map.l2() - (SegmentName::History.tag().len() + 5)
    }

    /// The tag-excluded content of `name`'s segment, i.e. the exact bytes
    /// the segment contributes to the prompt.
    #[must_use]
    pub fn segment_content(&self, name: SegmentName) -> Option<&str> {
        self.inner_range(name)
            .and_then(|range| self.prompt.get(range))
    }

    /// Blake3 hash of the tag-excluded segment content (the identity the
    /// context-epoch baseline pins).
    #[must_use]
    pub fn segment_hash(&self, name: SegmentName) -> Option<ContentHash> {
        self.segment_content(name)
            .map(|content| ContentHash::from_bytes(content.as_bytes()))
    }

    /// The frozen-segment hashes in canonical order — the epoch baseline
    /// input.
    #[must_use]
    pub fn frozen_hashes(&self) -> Vec<(SegmentName, ContentHash)> {
        SegmentName::FROZEN
            .iter()
            .filter_map(|&name| self.segment_hash(name).map(|hash| (name, hash)))
            .collect()
    }

    fn inner_range(&self, name: SegmentName) -> Option<std::ops::Range<usize>> {
        let span = self.map.span(name)?;
        let tag = name.tag();
        let header = tag.len() + 3; // "<tag>\n"
        let close = tag.len() + 4; // "</tag>\n"
        // Non-empty blocks end with "\n</tag>\n"; an empty block is the two
        // fences concatenated with no separator newline.
        let footer = if span.len == header + close {
            close
        } else {
            close + 1
        };
        Some(span.start + header..span.end() - footer)
    }
}

/// The Prompt Idempotency Manager's composition entry point.
pub struct PromptBuilder;

impl PromptBuilder {
    /// Byte bound on the volatile tail. The tail is one user message; a
    /// prompt whose tail exceeds this is rejected typed rather than
    /// shipped unbounded.
    pub const MAX_VOLATILE_TAIL_BYTES: usize = 64 * 1024;

    /// Composes `spec` into the fixed layout. Pure: the same spec renders
    /// the same bytes every time.
    pub fn render(spec: &ContextSpec) -> Result<RenderedPrompt, BuildError> {
        let rules = normalize_slots(&spec.rules);
        let skills = normalize_slots(&spec.skills);
        scan_frozen(SegmentName::System, &spec.system)?;
        scan_frozen(SegmentName::ToolSchema, &spec.base_schema)?;
        for slot in rules.iter().chain(skills.iter()) {
            scan_frozen(SegmentName::Rules, &slot.content)?;
        }
        for slot in &skills {
            scan_frozen(SegmentName::Skills, &slot.content)?;
        }
        scan_frozen(SegmentName::Repomap, &spec.repomap)?;
        if spec.last_user_message.len() > Self::MAX_VOLATILE_TAIL_BYTES {
            return Err(BuildError::OversizedTail {
                len: spec.last_user_message.len(),
                max: Self::MAX_VOLATILE_TAIL_BYTES,
            });
        }

        let system_inner = join_non_empty([&spec.system, &spec.effort.render_lines()]);
        let rules_inner = join_slots(&rules);
        let skills_inner = join_slots(&skills);
        let history_inner = render_history(&spec.history);

        let blocks = [
            (
                SegmentName::System,
                Stability::Stable,
                block(SegmentName::System.tag(), &system_inner),
            ),
            (
                SegmentName::ToolSchema,
                Stability::Stable,
                block(SegmentName::ToolSchema.tag(), &spec.base_schema),
            ),
            (
                SegmentName::Rules,
                Stability::Stable,
                block(SegmentName::Rules.tag(), &rules_inner),
            ),
            (
                SegmentName::Skills,
                Stability::Stable,
                block(SegmentName::Skills.tag(), &skills_inner),
            ),
            (
                SegmentName::Repomap,
                Stability::Stable,
                block(SegmentName::Repomap.tag(), &spec.repomap),
            ),
            (
                SegmentName::History,
                Stability::Stable,
                block(SegmentName::History.tag(), &history_inner),
            ),
            (
                SegmentName::VolatileTail,
                Stability::Volatile,
                block(SegmentName::VolatileTail.tag(), &spec.last_user_message),
            ),
        ];
        let mut prompt = String::new();
        let mut spans = Vec::with_capacity(blocks.len());
        for (name, stability, text) in blocks {
            let start = prompt.len();
            prompt.push_str(&text);
            spans.push(SegmentSpan {
                name,
                start,
                len: text.len(),
                stability,
            });
        }
        let map = SegmentMap::new(spans);
        let stable_prefix_hash = ContentHash::from_bytes(&prompt.as_bytes()[..map.l2()]);
        Ok(RenderedPrompt {
            prompt,
            map,
            stable_prefix_hash,
            beta_effort: spec.beta_effort.clone(),
        })
    }
}

fn scan_frozen(name: SegmentName, content: &str) -> Result<(), BuildError> {
    match scan_volatile(content) {
        Some((kind, byte_offset, evidence)) => Err(BuildError::Layout(LayoutViolation {
            segment: name,
            byte_offset,
            kind,
            evidence,
        })),
        None => Ok(()),
    }
}

/// Sorts slots by `(id, content)` and normalizes JSON bodies to sorted-key
/// compact form, so insertion order and unsorted keys cannot leak into
/// the prefix.
fn normalize_slots(slots: &[SlotSegment]) -> Vec<SlotSegment> {
    let mut normalized: Vec<SlotSegment> = slots
        .iter()
        .map(|slot| SlotSegment {
            id: slot.id.clone(),
            content: normalize_json(&slot.content),
        })
        .collect();
    normalized.sort_by(|a, b| (&a.id, &a.content).cmp(&(&b.id, &b.content)));
    normalized
}

fn normalize_json(content: &str) -> String {
    let trimmed = content.trim_start();
    if (trimmed.starts_with('{') || trimmed.starts_with('['))
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed)
    {
        // Infallible for parsed values.
        serde_json::to_string(&value).unwrap_or_default()
    } else {
        content.to_owned()
    }
}

fn join_non_empty<'a>(parts: impl IntoIterator<Item = &'a String>) -> String {
    parts
        .into_iter()
        .filter(|part| !part.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
}

fn join_slots(slots: &[SlotSegment]) -> String {
    slots
        .iter()
        .map(|slot| slot.content.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn block(tag: &str, inner: &str) -> String {
    if inner.is_empty() {
        format!("<{tag}>\n</{tag}>\n")
    } else {
        format!("<{tag}>\n{inner}\n</{tag}>\n")
    }
}

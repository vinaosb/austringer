//! The folded-view reconstruction (plan todo 13, Grok pattern): the
//! loader-side output of the segment store. [`FoldedView`] is what
//! [`SegmentStore::load_folded`](super::segments::SegmentStore::load_folded)
//! produces — the segments re-joined into an auditable render of what
//! compaction preserved.

use super::segments::{SegmentKind, SegmentRecord};

/// The reconstructed folded view loaded from disk.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FoldedView {
    /// The segments in index order.
    pub segments: Vec<SegmentRecord>,
}

impl FoldedView {
    /// Renders the folded view: summaries, excerpts, and retained
    /// history re-joined deterministically.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::from("<folded_view>\n");
        for record in &self.segments {
            match record.kind {
                SegmentKind::Summary | SegmentKind::SealedSpan => {
                    out.push_str(&record.summary);
                    out.push('\n');
                }
                SegmentKind::RetainedSpan => {
                    out.push_str(&crate::context::render_history(
                        record.events.as_deref().unwrap_or_default(),
                    ));
                    out.push('\n');
                }
            }
        }
        out.push_str("</folded_view>");
        out
    }
}

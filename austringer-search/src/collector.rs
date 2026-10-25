//! DocAddress collector: enumerates every live document so the incremental
//! refresh can diff disk state against index state.

use std::collections::HashMap;

use tantivy::collector::{Collector, SegmentCollector};
use tantivy::{DocAddress, DocId, SegmentOrdinal, SegmentReader};

/// Collects all matching [`DocAddress`]es (unordered).
#[derive(Debug, Default)]
pub(crate) struct DocAddrs {
    addresses: HashMap<SegmentOrdinal, Vec<DocId>>,
}

impl DocAddrs {
    /// The collected addresses.
    pub fn into_flat(self) -> Vec<DocAddress> {
        let mut out: Vec<DocAddress> = self
            .addresses
            .into_iter()
            .flat_map(|(segment_ord, docs)| {
                docs.into_iter()
                    .map(move |doc| DocAddress::new(segment_ord, doc))
            })
            .collect();
        out.sort();
        out
    }
}

impl Collector for DocAddrs {
    type Fruit = DocAddrs;
    type Child = DocAddrsChild;

    fn for_segment(
        &self,
        segment_local_id: SegmentOrdinal,
        _segment: &SegmentReader,
    ) -> tantivy::Result<Self::Child> {
        Ok(DocAddrsChild {
            segment_ord: segment_local_id,
            docs: Vec::new(),
        })
    }

    fn requires_scoring(&self) -> bool {
        false
    }

    fn merge_fruits(&self, mut segment_fruits: Vec<DocAddrs>) -> tantivy::Result<Self::Fruit> {
        let mut merged = DocAddrs::default();
        for fruit in segment_fruits.drain(..) {
            merged.addresses.extend(fruit.addresses);
        }
        Ok(merged)
    }
}

#[derive(Debug)]
pub(crate) struct DocAddrsChild {
    segment_ord: SegmentOrdinal,
    docs: Vec<DocId>,
}

impl SegmentCollector for DocAddrsChild {
    type Fruit = DocAddrs;

    fn collect(&mut self, doc: DocId, _score: tantivy::Score) {
        self.docs.push(doc);
    }

    fn harvest(self) -> Self::Fruit {
        let mut addresses = HashMap::new();
        addresses.insert(self.segment_ord, self.docs);
        DocAddrs { addresses }
    }
}

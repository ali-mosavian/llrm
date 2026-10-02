//! The type tree of a module's `!tbaa` metadata, built once: for each type
//! node, the names of its ancestors, nearest first, the root last. LLVM's
//! TypeBasedAA reads the same parent links; two accesses are apart only when
//! their types share a root and neither is an ancestor of the other.

use crate::module::{MetadataId, MetadataNode, MetadataOperand};

/// Each metadata node's ancestry as a type node, empty where it has none.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Tbaa {
    lineages: Vec<Vec<String>>,
}

impl Tbaa {
    pub fn of(metadata: &[MetadataNode]) -> Self {
        let mut lineages: Vec<Option<Vec<String>>> = vec![None; metadata.len()];
        for at in 0..metadata.len() {
            Self::lineage_of(metadata, at, &mut lineages, 0);
        }
        Self { lineages: lineages.into_iter().map(Option::unwrap_or_default).collect() }
    }

    /// The ancestors of type node `at`, memoized; none past a cycle.
    fn lineage_of(metadata: &[MetadataNode], at: usize, lineages: &mut Vec<Option<Vec<String>>>, depth: usize) -> Vec<String> {
        if let Some(known) = &lineages[at] {
            return known.clone();
        }
        // A type has no more ancestors than the module has nodes.
        if depth > metadata.len() {
            return Vec::new();
        }
        let lineage = match metadata[at].operands.get(1) {
            Some(MetadataOperand::Node(parent)) => {
                let parent = parent.0 as usize;
                match metadata.get(parent).and_then(|node| node.operands.first()) {
                    Some(MetadataOperand::String(name)) => {
                        let mut names = vec![name.clone()];
                        names.extend(Self::lineage_of(metadata, parent, lineages, depth + 1));
                        names
                    }
                    _ => Vec::new(),
                }
            }
            _ => Vec::new(),
        };
        lineages[at] = Some(lineage.clone());
        lineage
    }

    /// The ancestors of the access type tag `tag` names, nearest first, the root last.
    pub fn of_tag(&self, metadata: &[MetadataNode], tag: MetadataId) -> &[String] {
        match metadata.get(tag.0 as usize).and_then(|node| node.operands.first()) {
            Some(MetadataOperand::Node(ty)) => self.lineages.get(ty.0 as usize).map_or(&[], Vec::as_slice),
            _ => &[],
        }
    }
}

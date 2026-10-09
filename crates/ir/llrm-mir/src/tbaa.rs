//! The type tree of a module's `!tbaa` metadata, built once: for each type
//! node, the names of its ancestors, nearest first, the root last. LLVM's
//! TypeBasedAA reads the same parent links; two accesses are apart only when
//! their types share a root and neither is an ancestor of the other.

use crate::module::{MetadataId, MetadataNode, MetadataOperand};

/// Each metadata node's ancestry as a type node, empty where it has none.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Tbaa {
    lineages: Vec<Vec<String>>,
    /// The same lineages and the type names, each shared: asking for one is a count, not a copy of its strings.
    shared: Vec<std::rc::Rc<[String]>>,
    names: Vec<Option<std::rc::Rc<str>>>,
    empty: std::rc::Rc<[String]>,
}

thread_local! {
    static BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many type trees this thread has built, for a test that none is built per access.
pub fn built() -> usize {
    BUILT.with(std::cell::Cell::get)
}

impl Tbaa {
    pub fn of(metadata: &[MetadataNode]) -> Self {
        BUILT.with(|built| built.set(built.get() + 1));
        let mut lineages: Vec<Option<Vec<String>>> = vec![None; metadata.len()];
        for at in 0..metadata.len() {
            Self::lineage_of(metadata, at, &mut lineages, 0);
        }
        let lineages: Vec<Vec<String>> = lineages.into_iter().map(Option::unwrap_or_default).collect();
        let shared = lineages.iter().map(|one| std::rc::Rc::from(one.as_slice())).collect();
        let names = metadata
            .iter()
            .map(|node| match node.operands.first() {
                Some(MetadataOperand::String(name)) => Some(std::rc::Rc::from(name.as_str())),
                _ => None,
            })
            .collect();
        Self { lineages, shared, names, empty: std::rc::Rc::from(Vec::new()) }
    }

    /// `of_tag`, shared.
    pub fn shared_of_tag(&self, metadata: &[MetadataNode], tag: MetadataId) -> std::rc::Rc<[String]> {
        match metadata.get(tag.0 as usize).and_then(|node| node.operands.first()) {
            Some(MetadataOperand::Node(ty)) => self.shared.get(ty.0 as usize).map_or_else(|| self.empty.clone(), std::rc::Rc::clone),
            _ => self.empty.clone(),
        }
    }

    /// The name of the access type `tag` names, shared.
    pub fn name_of_tag(&self, metadata: &[MetadataNode], tag: MetadataId) -> Option<std::rc::Rc<str>> {
        let Some(MetadataOperand::Node(ty)) = metadata.get(tag.0 as usize)?.operands.first() else { return None };
        self.names.get(ty.0 as usize)?.clone()
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

    /// `of_tag` where there is no tree: the ancestors of the type `tag` names, walked up from it, so
    /// that asking costs its depth and not the module's metadata (the tree is built from all of it).
    pub fn chain(metadata: &[MetadataNode], tag: MetadataId) -> Vec<String> {
        let Some(MetadataOperand::Node(ty)) = metadata.get(tag.0 as usize).and_then(|node| node.operands.first()) else { return Vec::new() };
        let (mut names, mut at) = (Vec::new(), ty.0 as usize);
        // A type has no more ancestors than the module has nodes.
        while names.len() <= metadata.len() {
            let Some(MetadataOperand::Node(parent)) = metadata.get(at).and_then(|node| node.operands.get(1)) else { break };
            let parent = parent.0 as usize;
            let Some(MetadataOperand::String(name)) = metadata.get(parent).and_then(|node| node.operands.first()) else { break };
            names.push(name.clone());
            at = parent;
        }
        names
    }

    /// The ancestors of the access type tag `tag` names, nearest first, the root last.
    pub fn of_tag(&self, metadata: &[MetadataNode], tag: MetadataId) -> &[String] {
        match metadata.get(tag.0 as usize).and_then(|node| node.operands.first()) {
            Some(MetadataOperand::Node(ty)) => self.lineages.get(ty.0 as usize).map_or(&[], Vec::as_slice),
            _ => &[],
        }
    }
}

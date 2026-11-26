//! The edges of a function's blocks, kept as their jumps change (LLVM keeps
//! them on the block: `addSuccessor` / `removeSuccessor` update both ends).
//! Branch folding and tail merging each read the edges their own way and share
//! the bookkeeping.

use crate::mfunc::Block;

const ABSENT: usize = usize::MAX;

/// The edges of every block by block id.
pub(crate) struct Graph {
    /// Sorted, without repeats.
    successors: Vec<Vec<u32>>,
    /// The blocks that name or fall into the block, in no order; the layout
    /// order is put on them when they are asked for.
    predecessors: Vec<Vec<u32>>,
}

impl Graph {
    /// The edges of `blocks`; `read` gives the sorted edges out of the block at
    /// an index.
    pub(crate) fn build(
        blocks: &[Block],
        mut read: impl FnMut(usize, &mut Vec<u32>),
    ) -> Graph {
        let size = blocks.iter().map(|block| block.id as usize + 1).max().unwrap_or(0);
        let mut graph = Graph { successors: vec![Vec::new(); size], predecessors: vec![Vec::new(); size] };
        let mut out = Vec::new();
        for (at, block) in blocks.iter().enumerate() {
            read(at, &mut out);
            for &to in &out {
                // a jump to a block that is not there reaches no one
                if let Some(into) = graph.predecessors.get_mut(to as usize) {
                    into.push(block.id);
                }
            }
            graph.successors[block.id as usize] = out.clone();
        }
        graph
    }

    /// The jumps of these blocks changed: their edges are read again, both
    /// ends. Blocks that were not there are added. `at` gives a block's index.
    pub(crate) fn relink(
        &mut self,
        ids: &[u32],
        mut at: impl FnMut(u32) -> usize,
        mut read: impl FnMut(usize, &mut Vec<u32>),
    ) {
        let size = ids.iter().map(|&id| id as usize + 1).fold(self.successors.len(), usize::max);
        self.successors.resize(size, Vec::new());
        self.predecessors.resize(size, Vec::new());
        let mut out = Vec::new();
        for &id in ids {
            read(at(id), &mut out);
            let old = std::mem::take(&mut self.successors[id as usize]);
            for &to in old.iter().filter(|to| !out.contains(to)) {
                if let Some(into) = self.predecessors.get_mut(to as usize) {
                    into.retain(|&from| from != id);
                }
            }
            for &to in out.iter().filter(|to| !old.contains(to)) {
                if let Some(into) = self.predecessors.get_mut(to as usize) {
                    into.push(id);
                }
            }
            self.successors[id as usize] = out.clone();
        }
    }

    /// The block is gone: it no longer reaches anyone.
    pub(crate) fn forget(
        &mut self,
        id: u32,
    ) {
        let Some(old) = self.successors.get_mut(id as usize).map(std::mem::take) else { return };
        for to in old {
            if let Some(into) = self.predecessors.get_mut(to as usize) {
                into.retain(|&from| from != id);
            }
        }
    }

    pub(crate) fn successors(
        &self,
        id: u32,
    ) -> &[u32] {
        self.successors.get(id as usize).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn predecessors(
        &self,
        id: u32,
    ) -> &[u32] {
        self.predecessors.get(id as usize).map_or(&[], Vec::as_slice)
    }
}

/// Block id to its place in the layout, built on the first ask and dropped when
/// a block is inserted, removed or moved.
#[derive(Default)]
pub(crate) struct Layout(Option<Vec<usize>>);

impl Layout {
    pub(crate) fn invalidate(&mut self) {
        self.0 = None;
    }

    /// The index of block `id`; `built` runs when the table is made.
    pub(crate) fn at(
        &mut self,
        blocks: &[Block],
        id: u32,
        built: impl FnOnce(),
    ) -> usize {
        let positions = self
            .0
            .get_or_insert_with(
                || {
                    built();
                    let size = blocks.iter().map(|block| block.id as usize + 1).max().unwrap_or(0);
                    let mut position = vec![ABSENT; size];
                    for (at, block) in blocks.iter().enumerate() {
                        position[block.id as usize] = at;
                    }
                    position
                },
            );
        positions.get(id as usize).copied().filter(|&at| at != ABSENT).expect("a block that exists")
    }
}

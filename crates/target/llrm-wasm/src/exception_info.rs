//! `WebAssemblyExceptionInfo`: the regions of a function an EH pad heads.
//!
//! # How it works
//! An exception is its pad and every block the pad dominates that is reached
//! from it; one that holds the pad of another is its parent. Exceptions are
//! found innermost first (the dominator tree in post order), so a pad met
//! while walking an outer one already owns its blocks and the walk continues
//! from its dominance frontier. Cost: O(blocks + edges) for the frontier and
//! O(blocks) per nesting level for the block sets.
//!
//! # See also
//! `WebAssemblyExceptionInfo.cpp` (`recalculate`, `discoverAndMapException`),
//! `WebAssemblySortRegion.cpp`, pin d1106deb71cc.

use llrm_mir::domtree::DomTree;
use llrm_mir::module::BlockId;

use crate::cfg::Cfg;
use crate::mir::Function;

/// One exception: its pad and its blocks, the sub-exceptions' included.
#[derive(Debug)]
pub struct Exception {
    pub header: usize,
    pub parent: Option<usize>,
    pub blocks: Vec<usize>,
}

#[derive(Debug, Default)]
pub struct Exceptions {
    pub all: Vec<Exception>,
    /// The innermost exception of each block (`BBMap`).
    of_block: Vec<Option<usize>>,
}

/// The dominance frontier of each block (Cooper, Harvey and Kennedy).
fn frontiers(
    cfg: &Cfg,
    tree: &DomTree,
    n: usize,
) -> Vec<Vec<usize>> {
    let mut frontier = vec![Vec::new(); n];
    for block in 0..n {
        let preds: Vec<usize> = cfg.predecessors_of(block).collect();
        if preds.len() < 2 {
            continue;
        }
        let Some(idom) = tree.immediate_dominator(BlockId(block as u32)) else { continue };
        for pred in preds {
            let mut runner = Some(BlockId(pred as u32));
            while let Some(at) = runner.filter(|&at| at != idom) {
                if !frontier[at.0 as usize].contains(&block) {
                    frontier[at.0 as usize].push(block);
                }
                runner = tree.immediate_dominator(at);
            }
        }
    }
    frontier
}

/// The dominator tree's blocks, children before their parent.
fn post_order(
    tree: &DomTree,
    root: BlockId,
) -> Vec<usize> {
    let mut order = Vec::new();
    let mut stack = vec![(root, false)];
    while let Some((block, done)) = stack.pop() {
        if done {
            order.push(block.0 as usize);
            continue;
        }
        stack.push((block, true));
        // The pin walks the children in order; the first is visited first.
        for child in tree.children(Some(block)).into_iter().rev() {
            stack.push((child, false));
        }
    }
    order
}

impl Exceptions {
    /// `recalculate`; none for a function that has no pad.
    pub fn of(
        f: &Function,
        cfg: &Cfg,
        tree: &DomTree,
    ) -> Exceptions {
        let n = f.blocks.len();
        let mut found = Exceptions { all: Vec::new(), of_block: vec![None; n] };
        if !f.personality || !(0..n).any(|block| f.is_eh_pad(block)) {
            return found;
        }
        let frontier = frontiers(cfg, tree, n);
        let order = post_order(tree, BlockId(0));
        for &pad in order.iter().filter(|&&block| f.is_eh_pad(block)) {
            let id = found.all.len();
            found.all.push(Exception { header: pad, parent: None, blocks: Vec::new() });
            found.discover(id, f, tree, &frontier);
        }
        // Each block joins its exception and every enclosing one.
        for &block in order.iter().rev() {
            let mut at = found.of_block[block];
            while let Some(exception) = at {
                found.all[exception].blocks.push(block);
                at = found.all[exception].parent;
            }
        }
        found
    }

    /// `discoverAndMapException`.
    fn discover(
        &mut self,
        id: usize,
        f: &Function,
        tree: &DomTree,
        frontier: &[Vec<usize>],
    ) {
        let pad = self.all[id].header;
        let dominated = |block: usize| tree.dominates(BlockId(pad as u32), BlockId(block as u32));
        let mut work = vec![pad];
        while let Some(block) = work.pop() {
            if let Some(sub) = self.outermost(block) {
                if sub != id {
                    self.all[sub].parent = Some(id);
                    let sub_pad = self.all[sub].header;
                    work.extend(frontier[sub_pad].iter().copied().filter(|&to| dominated(to)));
                }
                continue;
            }
            self.of_block[block] = Some(id);
            work.extend(f.succs[block].iter().copied().filter(|&to| dominated(to)));
        }
    }

    /// `getOutermostException`.
    fn outermost(
        &self,
        block: usize,
    ) -> Option<usize> {
        let mut at = self.of_block[block]?;
        while let Some(parent) = self.all[at].parent {
            at = parent;
        }
        Some(at)
    }

    /// `getExceptionFor`: the innermost exception of `block`.
    pub fn exception_for(
        &self,
        block: usize,
    ) -> Option<usize> {
        self.of_block.get(block).copied().flatten()
    }

    pub fn contains(
        &self,
        exception: usize,
        block: usize,
    ) -> bool {
        self.all[exception].blocks.contains(&block)
    }

    /// `getBottom`: the last block of the exception in the layout.
    pub fn bottom(
        &self,
        exception: usize,
    ) -> usize {
        self.all[exception]
            .blocks
            .iter()
            .copied()
            .max()
            .unwrap_or(self.all[exception].header)
            .max(self.all[exception].header)
    }
}

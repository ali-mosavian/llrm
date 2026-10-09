//! The dominator tree, as LLVM's `DominatorTree`: by Cooper, Harvey and
//! Kennedy's iteration over reverse postorder. Unreachable blocks are in no
//! tree, and, as in LLVM, everything dominates them.

use crate::module::{BlockId, Function, InstId};

/// Absent from a table indexed by block: an unreachable block.
const NONE: u32 = u32::MAX;

/// Tables by block id, which are dense, so a query is an index and no hash: `dominates` is asked
/// for every pair a pass compares, and walking up the tree made it as deep as the loops nest.
#[derive(Clone, Debug, PartialEq)]
pub struct DominatorTree {
    /// Each reachable block's immediate dominator; the entry's is itself.
    idom: Vec<u32>,
    /// Reverse postorder position of each reachable block.
    order: Vec<u32>,
    /// A preorder number of the dominator tree, and the last one in each block's subtree: `a`
    /// dominates `b` where `b`'s lies between `a`'s two.
    enter: Vec<u32>,
    last: Vec<u32>,
}

impl DominatorTree {
    pub fn new(function: &Function) -> Self {
        let Some(entry) = function.entry() else {
            return Self { idom: Vec::new(), order: Vec::new(), enter: Vec::new(), last: Vec::new() };
        };
        let postorder = postorder(function, entry);
        let rpo: Vec<BlockId> = postorder.iter().rev().copied().collect();
        let size = rpo.iter().map(|block| block.0 as usize).max().map_or(0, |most| most + 1);
        let mut order = vec![NONE; size];
        for (at, block) in rpo.iter().enumerate() {
            order[block.0 as usize] = at as u32;
        }
        let mut idom = vec![NONE; size];
        idom[entry.0 as usize] = entry.0;
        let mut changed = true;
        while changed {
            changed = false;
            for &block in rpo.iter().skip(1) {
                let mut chosen: Option<u32> = None;
                for predecessor in function.predecessors(block) {
                    if idom.get(predecessor.0 as usize).is_none_or(|one| *one == NONE) {
                        continue;
                    }
                    chosen = Some(match chosen {
                        None => predecessor.0,
                        Some(other) => intersect(&idom, &order, predecessor.0, other),
                    });
                }
                if let Some(chosen) = chosen
                    && idom[block.0 as usize] != chosen
                {
                    idom[block.0 as usize] = chosen;
                    changed = true;
                }
            }
        }
        // A block follows its dominator in reverse postorder, so numbering in that order gives each
        // subtree a contiguous run: sizes bottom up, then each block's number from its parent's.
        let mut subtree = vec![1_u32; size];
        for block in rpo.iter().skip(1).rev() {
            subtree[idom[block.0 as usize] as usize] += subtree[block.0 as usize];
        }
        let (mut enter, mut last, mut free) = (vec![NONE; size], vec![NONE; size], vec![0_u32; size]);
        enter[entry.0 as usize] = 0;
        free[entry.0 as usize] = 1;
        for block in rpo.iter().skip(1) {
            let (at, parent) = (block.0 as usize, idom[block.0 as usize] as usize);
            enter[at] = free[parent];
            free[parent] += subtree[at];
            free[at] = enter[at] + 1;
        }
        for block in &rpo {
            last[block.0 as usize] = enter[block.0 as usize] + subtree[block.0 as usize] - 1;
        }
        Self { idom, order, enter, last }
    }

    pub fn is_reachable(
        &self,
        block: BlockId,
    ) -> bool {
        self.order.get(block.0 as usize).is_some_and(|one| *one != NONE)
    }

    pub fn immediate_dominator(
        &self,
        block: BlockId,
    ) -> Option<BlockId> {
        self.idom.get(block.0 as usize).copied().filter(|one| *one != NONE && *one != block.0).map(BlockId)
    }

    /// Whether every path from the entry to `b` passes `a`. An unreachable
    /// `b` is dominated by everything.
    pub fn dominates(
        &self,
        a: BlockId,
        b: BlockId,
    ) -> bool {
        if !self.is_reachable(b) {
            return true;
        }
        if !self.is_reachable(a) {
            return false;
        }
        let (a, b) = (a.0 as usize, b.0 as usize);
        self.enter[a] <= self.enter[b] && self.enter[b] <= self.last[a]
    }

    /// Whether `def` comes before `user` on every path to `user`.
    pub fn instruction_dominates(
        &self,
        function: &Function,
        def: InstId,
        user: InstId,
    ) -> bool {
        let (Some(a), Some(b)) = (function.parent(def), function.parent(user)) else { return false };
        if a != b {
            return self.dominates(a, b);
        }
        if !self.is_reachable(b) {
            return true;
        }
        SCANS.with(|scans| scans.set(scans.get() + 1));
        let list = function.block(a).instructions();
        list.iter().position(|one| *one == def) < list.iter().position(|one| *one == user)
    }

    /// `instruction_dominates`, where `position` is each instruction's index in its block
    /// (`Function::positions`): no scan of the block for two in the same one.
    pub fn instruction_dominates_at(
        &self,
        function: &Function,
        def: InstId,
        user: InstId,
        position: &[u32],
    ) -> bool {
        let (Some(a), Some(b)) = (function.parent(def), function.parent(user)) else { return false };
        if a != b {
            return self.dominates(a, b);
        }
        !self.is_reachable(b) || position[def.0 as usize] < position[user.0 as usize]
    }
}

thread_local! {
    static SCANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has scanned a block's instructions to order two of them, for a test that the
/// verifier does not.
pub fn scans() -> usize {
    SCANS.with(std::cell::Cell::get)
}

fn intersect(
    idom: &[u32],
    order: &[u32],
    mut a: u32,
    mut b: u32,
) -> u32 {
    while a != b {
        while order[a as usize] > order[b as usize] {
            a = idom[a as usize];
        }
        while order[b as usize] > order[a as usize] {
            b = idom[b as usize];
        }
    }
    a
}

/// The blocks reachable from `entry`, in postorder, without recursion.
fn postorder(
    function: &Function,
    entry: BlockId,
) -> Vec<BlockId> {
    let mut out = Vec::new();
    let mut seen = crate::hash::HashSet::from_iter([entry]);
    let mut stack = vec![(entry, function.successors(entry), 0)];
    while let Some((block, successors, next)) = stack.last_mut() {
        if let Some(&successor) = successors.get(*next) {
            *next += 1;
            if seen.insert(successor) {
                let successors = function.successors(successor);
                stack.push((successor, successors, 0));
            }
        } else {
            out.push(*block);
            stack.pop();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    fn function(text: &str) -> Function {
        let module = crate::parse::module(text).unwrap_or_else(|error| panic!("{error}\n{text}"));
        module
            .functions()
            .find(|(_, _, function)| !function.is_declaration())
            .map(|(_, _, function)| function.clone())
            .expect("a body")
    }

    /// `a` dominates `b` where `b` cannot be reached from the entry once `a` is gone.
    fn by_definition(
        function: &Function,
        a: BlockId,
        b: BlockId,
    ) -> bool {
        let entry = function.entry().expect("an entry");
        if a == entry {
            return true;
        }
        let (mut seen, mut work) = (crate::hash::HashSet::from_iter([entry]), vec![entry]);
        while let Some(at) = work.pop() {
            for next in function.successors(at) {
                if next != a && seen.insert(next) {
                    work.push(next);
                }
            }
        }
        !seen.contains(&b)
    }

    #[test]
    fn dominance_is_what_the_definition_says_for_every_pair() {
        let function = function(
            "define void @f(i1 %c) {\nentry:\n  br i1 %c, label %a, label %b\na:\n  br label %join\nb:\n  br i1 %c, label %join, label %loop\nloop:\n  br i1 %c, label %loop, label %out\njoin:\n  br label %out\nout:\n  ret void\n}\n",
        );
        let tree = DominatorTree::new(&function);
        let blocks: Vec<BlockId> = function.layout().to_vec();
        for &a in &blocks {
            for &b in &blocks {
                assert_eq!(tree.dominates(a, b), by_definition(&function, a, b), "{a:?} over {b:?}");
            }
        }
    }

    /// Asking was a walk up the tree through a hash map, as deep as the loops nest: 45% of a compile of
    /// 60 sequential loops, and 301 s of one of 200 (#394). It is a comparison of two numbers.
    #[test]
    fn asking_whether_a_block_dominates_does_not_walk_the_tree() {
        let depth = 20_000;
        let body: String = (0..depth).map(|at| format!("b{at}:\n  br label %b{}\n", at + 1)).collect();
        let function =
            function(&format!("define void @f() {{\nentry:\n  br label %b0\n{body}b{depth}:\n  ret void\n}}\n"));
        let tree = DominatorTree::new(&function);
        let last = *function.layout().last().expect("blocks");
        let start = Instant::now();
        let asked = function.layout().iter().filter(|&&at| tree.dominates(at, last)).count();
        assert_eq!(asked, depth + 2);
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "{:?} for {depth} questions of depth {depth}",
            start.elapsed()
        );
    }
}

//! The dominator tree, as LLVM's `DominatorTree`: by Cooper, Harvey and
//! Kennedy's iteration over reverse postorder. Unreachable blocks are in no
//! tree, and, as in LLVM, everything dominates them.
//!
//! A tree is also brought up to date from the edges the change log tells of
//! (`updated`): the blocks under the nearest common dominator of the blocks the
//! changed edges join are worked out again, and nothing else can move.

use crate::module::{BlockId, Change, Function, InstId, Operand};

/// Absent from a table indexed by block: an unreachable block.
const NONE: u32 = u32::MAX;

/// Tables by block id, which are dense, so a query is an index and no hash:
/// `dominates` is asked for every pair a pass compares, and walking up the tree
/// made it as deep as the loops nest.
#[derive(Clone, Debug)]
pub struct DominatorTree {
    /// Each reachable block's immediate dominator; the entry's is itself.
    idom: Vec<u32>,
    /// Reverse postorder position of each reachable block.
    order: Vec<u32>,
    /// A preorder number of the dominator tree, and the last one in each
    /// block's subtree: `a` dominates `b` where `b`'s lies between `a`'s
    /// two.
    enter: Vec<u32>,
    last: Vec<u32>,
}

/// Two trees are the same where every block has the same immediate dominator:
/// the numbers and the order are derived from them (a tree brought up to date
/// numbers its blocks in another order).
impl PartialEq for DominatorTree {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        let (short, long) = if self.idom.len() <= other.idom.len() { (self, other) } else { (other, self) };
        short.idom == long.idom[..short.idom.len()] && long.idom[short.idom.len()..].iter().all(|one| *one == NONE)
    }
}

impl DominatorTree {
    /// The tree of `function` after `changes`, from this one of the function
    /// before them, where they changed edges only inside the part of the
    /// tree they reach: the blocks under the nearest common dominator of
    /// the blocks the changed edges join, whose dominators are worked out again
    /// from there (Ramalingam and Reps: nothing outside that subtree can
    /// change). None where the changes did something else to the graph (a
    /// block gone, a branch moved), or the edges reach out of it.
    pub fn updated(
        &self,
        function: &Function,
        changes: &[Change],
    ) -> Option<Self> {
        let entry = function.entry()?;
        // What each terminator that was touched took, and now takes: its
        // operands now, with the changes told in the log undone from
        // the last.
        struct Touched {
            block: BlockId,
            operands: crate::module::Operands,
            inserted: bool,
        }
        let mut touched: crate::dense::IdMap<InstId, Touched> = crate::dense::IdMap::new();
        let blocks_of = |operands: &[Operand]| -> Vec<BlockId> {
            operands
                .iter()
                .filter_map(|one| if let Operand::Block(block) = one { Some(*block) } else { None })
                .collect()
        };
        let terminator = |inst: InstId| function.instruction(inst).opcode.is_terminator();
        for change in changes {
            match change {
                Change::BlockErased(_) => return None,
                Change::Moved { inst, .. } if terminator(*inst) => return None,
                Change::Operand { inst, .. } | Change::Operands { inst, .. } if terminator(*inst) => {
                    let block = function.parent(*inst)?;
                    touched.get_or_insert_with(*inst, || Touched {
                        block,
                        operands: function.instruction(*inst).operands.clone(),
                        inserted: false,
                    });
                }
                Change::Inserted { inst, block, .. } if terminator(*inst) => {
                    touched
                        .get_or_insert_with(*inst, || Touched {
                            block: *block,
                            operands: function.instruction(*inst).operands.clone(),
                            inserted: false,
                        })
                        .inserted = true;
                }
                Change::Erased { inst, block, .. } if terminator(*inst) => {
                    touched.get_or_insert_with(*inst, || Touched {
                        block: *block,
                        operands: function.instruction(*inst).operands.clone(),
                        inserted: false,
                    });
                }
                _ => {}
            }
        }
        for change in changes.iter().rev() {
            match change {
                Change::Operand { inst, index, was } => {
                    if let Some(one) = touched.get_mut(inst)
                        && let Some(slot) = one.operands.get_mut(*index as usize)
                    {
                        *slot = *was;
                    }
                }
                Change::Operands { inst, was } => {
                    if let Some(one) = touched.get_mut(inst) {
                        one.operands = was.iter().copied().collect();
                    }
                }
                _ => {}
            }
        }
        // The edges the touched terminators lost and gained.
        let mut joined: Vec<BlockId> = Vec::new();
        for (inst, one) in touched.iter() {
            let old = if one.inserted { Vec::new() } else { blocks_of(&one.operands) };
            let now = if function.is_erased(inst) || function.parent(inst).is_none() {
                Vec::new()
            } else {
                blocks_of(&function.instruction(inst).operands)
            };
            if old.iter().all(|one| now.contains(one)) && now.iter().all(|one| old.contains(one)) {
                continue;
            }
            joined.push(one.block);
            joined.extend(old.iter().chain(&now).copied());
        }
        let size = function.layout().iter().map(|block| block.0 as usize + 1).max().unwrap_or(0).max(self.idom.len());
        let known = |block: BlockId| self.idom.get(block.0 as usize).is_some_and(|one| *one != NONE);
        let mut idom = self.idom.clone();
        idom.resize(size, NONE);
        if joined.is_empty() {
            return Some(Self::numbered(idom, entry));
        }
        // The nearest common dominator of the blocks the changed edges join,
        // and of what dominated their targets.
        let mut root: Option<BlockId> = None;
        for &block in &joined {
            if !known(block) {
                continue;
            }
            let mut candidates = vec![block];
            if let Some(above) = self.immediate_dominator(block) {
                candidates.push(above);
            }
            for candidate in candidates {
                root = Some(match root {
                    None => candidate,
                    Some(one) => self.common(one, candidate),
                });
            }
        }
        let Some(root) = root else { return Some(Self::numbered(idom, entry)) };
        let (low, high) = (self.enter[root.0 as usize], self.last[root.0 as usize]);
        let inside = |block: BlockId| {
            !known(block) || (low <= self.enter[block.0 as usize] && self.enter[block.0 as usize] <= high)
        };
        // The blocks of the subtree (and any not reached before) that the root
        // reaches, in reverse postorder.
        let mut order = vec![NONE; size];
        let mut seen = crate::hash::HashSet::from_iter([root]);
        let mut post: Vec<BlockId> = Vec::new();
        let mut stack = vec![(root, function.successors(root), 0)];
        while let Some((block, successors, next)) = stack.last_mut() {
            if let Some(&successor) = successors.get(*next) {
                *next += 1;
                if inside(successor) && seen.insert(successor) {
                    let successors = function.successors(successor);
                    stack.push((successor, successors, 0));
                }
            } else {
                post.push(*block);
                stack.pop();
            }
        }
        let rpo: Vec<BlockId> = post.iter().rev().copied().collect();
        for (at, block) in rpo.iter().enumerate() {
            order[block.0 as usize] = at as u32;
        }
        let mut local = vec![NONE; size];
        local[root.0 as usize] = root.0;
        let mut changed = true;
        while changed {
            changed = false;
            for &block in rpo.iter().skip(1) {
                let mut chosen: Option<u32> = None;
                for predecessor in function.predecessors(block) {
                    if order[predecessor.0 as usize] == NONE {
                        // From outside the subtree: reached some other way than
                        // through the root,
                        // which dominated everything here, unless it is not
                        // reached at all.
                        if known(predecessor) && !inside(predecessor) {
                            return None;
                        }
                        continue;
                    }
                    if local[predecessor.0 as usize] == NONE {
                        continue;
                    }
                    chosen = Some(match chosen {
                        None => predecessor.0,
                        Some(other) => intersect(&local, &order, predecessor.0, other),
                    });
                }
                if let Some(chosen) = chosen
                    && local[block.0 as usize] != chosen
                {
                    local[block.0 as usize] = chosen;
                    changed = true;
                }
            }
        }
        // The blocks of the subtree not reached from the root any more are
        // reached from nowhere.
        let layout: Vec<BlockId> = function.layout().to_vec();
        for block in layout {
            if block == root || !inside(block) {
                continue;
            }
            idom[block.0 as usize] = if order[block.0 as usize] == NONE { NONE } else { local[block.0 as usize] };
        }
        Some(Self::numbered(idom, entry))
    }

    /// The nearest block that dominates both, in this tree.
    fn common(
        &self,
        a: BlockId,
        b: BlockId,
    ) -> BlockId {
        let mut a = a;
        while !self.dominates(a, b) {
            a = self.immediate_dominator(a).expect("the entry dominates every block");
        }
        a
    }

    /// The tree for these immediate dominators: each subtree numbered in a run,
    /// from the entry down.
    fn numbered(
        idom: Vec<u32>,
        entry: BlockId,
    ) -> Self {
        let size = idom.len();
        let mut first = vec![NONE; size];
        let mut next = vec![NONE; size];
        for at in (0..size).rev() {
            let parent = idom[at];
            if parent == NONE || at == entry.0 as usize {
                continue;
            }
            next[at] = first[parent as usize];
            first[parent as usize] = at as u32;
        }
        let (mut enter, mut last, mut order) = (vec![NONE; size], vec![NONE; size], vec![NONE; size]);
        let mut count = 0;
        let mut stack = vec![(entry.0 as usize, false)];
        while let Some((at, done)) = stack.pop() {
            if done {
                last[at] = count - 1;
                continue;
            }
            enter[at] = count;
            order[at] = count;
            count += 1;
            stack.push((at, true));
            let mut child = first[at];
            while child != NONE {
                stack.push((child as usize, false));
                child = next[child as usize];
            }
        }
        Self { idom, order, enter, last }
    }

    pub fn new(function: &Function) -> Self {
        crate::counts::bump(crate::counts::Event::Dominators);
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
        // A block follows its dominator in reverse postorder, so numbering in
        // that order gives each subtree a contiguous run: sizes bottom
        // up, then each block's number from its parent's.
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

    /// The preorder number of `block` in the tree and the last one in its
    /// subtree: `a` dominates `b` where `b`'s number lies between `a`'s two.
    /// None for an unreachable block.
    pub fn span(
        &self,
        block: BlockId,
    ) -> Option<(u32, u32)> {
        let at = block.0 as usize;
        self.is_reachable(block).then(|| (self.enter[at], self.last[at]))
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

    /// `instruction_dominates`, where `position` is each instruction's index in
    /// its block (`Function::positions`): no scan of the block for two in
    /// the same one.
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

/// How many times this thread has scanned a block's instructions to order two
/// of them, for a test that the verifier does not.
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
    let mut out = Vec::with_capacity(function.layout().len());
    let mut seen = crate::dense::IdSet::from_iter([entry]);
    let mut stack = Vec::with_capacity(function.layout().len().min(64));
    stack.push((entry, function.successors(entry), 0));
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

    /// `a` dominates `b` where `b` cannot be reached from the entry once `a` is
    /// gone.
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

    /// Asking was a walk up the tree through a hash map, as deep as the loops
    /// nest: 45% of a compile of 60 sequential loops, and 301 s of one of
    /// 200 (#394). It is a comparison of two numbers.
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

    /// The tree brought up to date from the edges a log tells of is the tree of
    /// the function afresh: over random graphs, each edited by a few
    /// branches taking other edges, a block between two, and a branch cut
    /// or made.
    #[test]
    fn a_tree_brought_up_to_date_is_the_tree_of_the_function_afresh() {
        use crate::module::Operand;
        let mut brought = 0;
        for seed in 1..400u64 {
            let mut state = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
            let mut next = move |most: u64| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state % most
            };
            let count = 3 + next(9) as usize;
            let mut text = String::from("define void @f(i1 %c) {\nentry:\n  br label %b0\n");
            for at in 0..count {
                text.push_str(&format!("b{at}:\n"));
                match next(4) {
                    0 => text.push_str("  ret void\n"),
                    1 => text.push_str(&format!("  br label %b{}\n", next(count as u64))),
                    _ => text.push_str(&format!(
                        "  br i1 %c, label %b{}, label %b{}\n",
                        next(count as u64),
                        next(count as u64)
                    )),
                }
            }
            text.push_str("}\n");
            let mut function = function(&text);
            let before = DominatorTree::new(&function);
            let mark = function.mark();
            let blocks: Vec<BlockId> = function.layout().to_vec();
            for _ in 0..1 + next(3) {
                let at = blocks[1 + next(count as u64) as usize];
                let last = function.terminator(at).expect("terminated");
                let mut operands = function.instruction(last).operands.clone();
                match operands.len() {
                    1 => operands[0] = Operand::Block(blocks[1 + next(count as u64) as usize]),
                    3 => {
                        let slot = 1 + next(2) as usize;
                        operands[slot] = Operand::Block(blocks[1 + next(count as u64) as usize]);
                    }
                    _ => continue,
                }
                function.set_operands(last, operands);
            }
            let changes = function.changes_since(mark).expect("on record").to_vec();
            let fresh = DominatorTree::new(&function);
            if let Some(updated) = before.updated(&function, &changes) {
                brought += 1;
                assert_eq!(updated, fresh, "seed {seed}: {text}{changes:?}");
                for &a in &blocks {
                    for &b in &blocks {
                        assert_eq!(updated.dominates(a, b), fresh.dominates(a, b), "seed {seed}: {a:?} over {b:?}");
                    }
                }
            }
        }
        assert!(brought > 300, "premise: the graphs were brought up to date, {brought} of 399");
    }
}

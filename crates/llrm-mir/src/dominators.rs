//! The dominator tree, as LLVM's `DominatorTree`: by Cooper, Harvey and
//! Kennedy's iteration over reverse postorder. Unreachable blocks are in no
//! tree, and, as in LLVM, everything dominates them.

use std::collections::HashMap;

use crate::module::{BlockId, Function, InstId};

#[derive(Clone, Debug, PartialEq)]
pub struct DominatorTree {
    /// Each reachable block's immediate dominator; the entry's is itself.
    idom: HashMap<BlockId, BlockId>,
    /// Reverse postorder position of each reachable block.
    order: HashMap<BlockId, usize>,
}

impl DominatorTree {
    pub fn new(function: &Function) -> Self {
        let Some(entry) = function.entry() else { return Self { idom: HashMap::new(), order: HashMap::new() } };
        let postorder = postorder(function, entry);
        let rpo: Vec<BlockId> = postorder.iter().rev().copied().collect();
        let order: HashMap<BlockId, usize> = rpo.iter().enumerate().map(|(at, block)| (*block, at)).collect();
        let mut idom: HashMap<BlockId, BlockId> = HashMap::from([(entry, entry)]);
        let mut changed = true;
        while changed {
            changed = false;
            for &block in rpo.iter().skip(1) {
                let mut chosen: Option<BlockId> = None;
                for predecessor in function.predecessors(block) {
                    if !idom.contains_key(&predecessor) {
                        continue;
                    }
                    chosen = Some(match chosen {
                        None => predecessor,
                        Some(other) => intersect(&idom, &order, predecessor, other),
                    });
                }
                if let Some(chosen) = chosen
                    && idom.get(&block) != Some(&chosen)
                {
                    idom.insert(block, chosen);
                    changed = true;
                }
            }
        }
        Self { idom, order }
    }

    pub fn is_reachable(&self, block: BlockId) -> bool {
        self.order.contains_key(&block)
    }

    pub fn immediate_dominator(&self, block: BlockId) -> Option<BlockId> {
        self.idom.get(&block).copied().filter(|one| *one != block)
    }

    /// Whether every path from the entry to `b` passes `a`. An unreachable
    /// `b` is dominated by everything.
    pub fn dominates(&self, a: BlockId, b: BlockId) -> bool {
        if !self.is_reachable(b) {
            return true;
        }
        if !self.is_reachable(a) {
            return false;
        }
        let mut at = b;
        loop {
            if at == a {
                return true;
            }
            match self.immediate_dominator(at) {
                Some(up) => at = up,
                None => return false,
            }
        }
    }

    /// Whether `def` comes before `user` on every path to `user`.
    pub fn instruction_dominates(&self, function: &Function, def: InstId, user: InstId) -> bool {
        let (Some(a), Some(b)) = (function.parent(def), function.parent(user)) else { return false };
        if a != b {
            return self.dominates(a, b);
        }
        if !self.is_reachable(b) {
            return true;
        }
        let list = function.block(a).instructions();
        list.iter().position(|one| *one == def) < list.iter().position(|one| *one == user)
    }
}

fn intersect(idom: &HashMap<BlockId, BlockId>, order: &HashMap<BlockId, usize>, mut a: BlockId, mut b: BlockId) -> BlockId {
    while a != b {
        while order[&a] > order[&b] {
            a = idom[&a];
        }
        while order[&b] > order[&a] {
            b = idom[&b];
        }
    }
    a
}

/// The blocks reachable from `entry`, in postorder, without recursion.
fn postorder(function: &Function, entry: BlockId) -> Vec<BlockId> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::from([entry]);
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

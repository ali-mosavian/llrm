//! The control-flow graph and dominator tree of a body, numbered by position:
//! what `MachineFunction` (block numbers, predecessors, successors) and
//! `MachineDominatorTree` (`getNode`, `getIDom`, `dominates`) are to LLVM's
//! live range construction.
//!
//! # How it works
//! A block's number is its position in `body.blocks`. Predecessors and
//! successors come from the blocks' `succ` addresses. The tree is the immediate
//! dominators by the iterative algorithm over a reverse postorder (Cooper,
//! Harvey, Kennedy), with a depth-first numbering so `dominates` is two
//! comparisons.
//!
//! DEVIATE (algorithm, no number): LLVM computes the tree with SemiNCA; the
//! tree is the same, the algorithm is the shorter one. Unreachable blocks have
//! no immediate dominator and dominate nothing.
//!
//! # Example
//! A diamond `0 -> {1, 2} -> 3`: the immediate dominator of 3 is 0, and 0
//! dominates every block.
//!
//! # Used by
//! `liverangecalc`, `liveintervals`.

use crate::model::lir::LirBody;
use crate::support::hash::HashMap;

/// Blocks by number, with their edges.
#[derive(Clone, Debug, Default)]
pub struct Cfg {
    pub preds: Vec<Vec<usize>>,
    pub succs: Vec<Vec<usize>>,
    pub entry: usize,
}

impl Cfg {
    pub fn of(body: &LirBody) -> Self {
        let number: HashMap<i64, usize> = body.blocks.iter().enumerate().map(|(n, block)| (block.at, n)).collect();
        let succs: Vec<Vec<usize>> = body
            .blocks
            .iter()
            .map(|block| block.succ.iter().filter_map(|at| number.get(at).copied()).collect())
            .collect();
        let mut preds = vec![Vec::new(); succs.len()];
        for (block, out) in succs.iter().enumerate() {
            for &to in out {
                if !preds[to].contains(&block) {
                    preds[to].push(block);
                }
            }
        }
        Self { preds, succs, entry: number.get(&body.entry).copied().unwrap_or(0) }
    }

    pub fn len(&self) -> usize {
        self.succs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.succs.is_empty()
    }
}

/// `MachineDominatorTree`: immediate dominators and the dominance test.
#[derive(Clone, Debug, Default)]
pub struct DomTree {
    idom: Vec<Option<usize>>,
    tin: Vec<u32>,
    tout: Vec<u32>,
    reachable: Vec<bool>,
}

impl DomTree {
    pub fn of(cfg: &Cfg) -> Self {
        let n = cfg.len();
        if n == 0 {
            return Self::default();
        }
        // Reverse postorder of the blocks reachable from the entry.
        let mut order: Vec<usize> = Vec::with_capacity(n);
        let mut visited = vec![false; n];
        let mut stack: Vec<(usize, usize)> = vec![(cfg.entry, 0)];
        visited[cfg.entry] = true;
        while let Some((block, next)) = stack.last().copied() {
            if let Some(&to) = cfg.succs[block].get(next) {
                stack.last_mut().expect("a frame").1 += 1;
                if !visited[to] {
                    visited[to] = true;
                    stack.push((to, 0));
                }
            } else {
                order.push(block);
                stack.pop();
            }
        }
        order.reverse();
        let mut position = vec![usize::MAX; n];
        for (at, &block) in order.iter().enumerate() {
            position[block] = at;
        }
        let mut idom: Vec<Option<usize>> = vec![None; n];
        idom[cfg.entry] = Some(cfg.entry);
        let intersect = |idom: &[Option<usize>], mut a: usize, mut b: usize| {
            while a != b {
                while position[a] > position[b] {
                    a = idom[a].expect("processed");
                }
                while position[b] > position[a] {
                    b = idom[b].expect("processed");
                }
            }
            a
        };
        let mut changed = true;
        while changed {
            changed = false;
            for &block in order.iter().skip(1) {
                let mut new: Option<usize> = None;
                for &pred in &cfg.preds[block] {
                    if idom[pred].is_none() {
                        continue;
                    }
                    new = Some(match new {
                        None => pred,
                        Some(current) => intersect(&idom, pred, current),
                    });
                }
                if new != idom[block] {
                    idom[block] = new;
                    changed = true;
                }
            }
        }
        idom[cfg.entry] = None;
        // Depth-first numbering of the tree.
        let mut children = vec![Vec::new(); n];
        for (block, parent) in idom.iter().enumerate() {
            if let Some(parent) = parent {
                children[*parent].push(block);
            }
        }
        let (mut tin, mut tout) = (vec![0u32; n], vec![0u32; n]);
        let mut clock = 0u32;
        let mut stack: Vec<(usize, usize)> = vec![(cfg.entry, 0)];
        tin[cfg.entry] = clock;
        clock += 1;
        while let Some((block, next)) = stack.last().copied() {
            if let Some(&child) = children[block].get(next) {
                stack.last_mut().expect("a frame").1 += 1;
                tin[child] = clock;
                clock += 1;
                stack.push((child, 0));
            } else {
                tout[block] = clock;
                clock += 1;
                stack.pop();
            }
        }
        Self { idom, tin, tout, reachable: visited }
    }

    /// `getIDom`: the immediate dominator, None for the entry and the
    /// unreachable.
    pub fn idom(
        &self,
        block: usize,
    ) -> Option<usize> {
        self.idom[block]
    }

    pub fn is_reachable(
        &self,
        block: usize,
    ) -> bool {
        self.reachable[block]
    }

    /// `findNearestCommonDominator`: the closest block that dominates both.
    pub fn find_nearest_common_dominator(
        &self,
        a: usize,
        b: usize,
    ) -> usize {
        let mut x = a;
        while !self.dominates(x, b) {
            x = self.idom[x].expect("a block with a dominator, or the entry dominates everything reachable");
        }
        x
    }

    /// `dominates(A, B)`: every path from the entry to `b` goes through `a`
    /// (a block dominates itself).
    pub fn dominates(
        &self,
        a: usize,
        b: usize,
    ) -> bool {
        self.reachable[a] && self.reachable[b] && self.tin[a] <= self.tin[b] && self.tout[b] <= self.tout[a]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(
        edges: &[(usize, usize)],
        n: usize,
    ) -> Cfg {
        let mut succs = vec![Vec::new(); n];
        let mut preds = vec![Vec::new(); n];
        for &(a, b) in edges {
            succs[a].push(b);
            preds[b].push(a);
        }
        Cfg { preds, succs, entry: 0 }
    }

    /// The dominators of each block by the definition (the sets, to a fixed
    /// point).
    fn dominator_sets(cfg: &Cfg) -> Vec<Option<std::collections::BTreeSet<usize>>> {
        let n = cfg.len();
        let all: std::collections::BTreeSet<usize> = (0..n).collect();
        let mut sets: Vec<std::collections::BTreeSet<usize>> = vec![all.clone(); n];
        sets[cfg.entry] = [cfg.entry].into_iter().collect();
        let mut changed = true;
        while changed {
            changed = false;
            for block in 0..n {
                if block == cfg.entry {
                    continue;
                }
                let mut new = all.clone();
                for &p in &cfg.preds[block] {
                    new = new.intersection(&sets[p]).copied().collect();
                }
                new.insert(block);
                if new != sets[block] {
                    sets[block] = new;
                    changed = true;
                }
            }
        }
        let tree = DomTree::of(cfg);
        (0..n).map(|b| tree.is_reachable(b).then(|| sets[b].clone())).collect()
    }

    /// The tree's `dominates` and `idom` against the dominator sets, on random
    /// graphs.
    #[test]
    fn test_the_tree_agrees_with_the_dominator_sets() {
        let mut seed = 5u64;
        let mut next = move |n: usize| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((seed >> 33) as usize) % n
        };
        let mut loops = 0;
        for _ in 0..300 {
            let n = next(9) + 2;
            let mut edges = Vec::new();
            for from in 0..n {
                for _ in 0..next(3) {
                    edges.push((from, next(n)));
                }
            }
            let cfg = cfg(&edges, n);
            let tree = DomTree::of(&cfg);
            let sets = dominator_sets(&cfg);
            loops += edges.iter().filter(|(a, b)| b <= a).count().min(1);
            for b in 0..n {
                for a in 0..n {
                    let want = sets[b].as_ref().is_some_and(|set| set.contains(&a)) && sets[a].is_some();
                    assert_eq!(tree.dominates(a, b), want, "{edges:?}: does {a} dominate {b}");
                }
                if let Some(set) = &sets[b] {
                    // the immediate dominator is the strict dominator with the
                    // most dominators
                    let strict = set
                        .iter()
                        .copied()
                        .filter(|d| *d != b)
                        .max_by_key(|d| sets[*d].as_ref().map_or(0, |s| s.len()));
                    assert_eq!(tree.idom(b), strict, "{edges:?}: idom of {b}");
                }
            }
        }
        assert!(loops > 50, "premise: graphs with back edges ({loops})");
    }
}

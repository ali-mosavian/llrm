//! The post-dominator tree of the blocks' CFG (`MachinePostDominatorTree`),
//! found once for the placement.

const UNSET: usize = usize::MAX;

pub(super) struct PostDom {
    enter: Vec<usize>,
    leave: Vec<usize>,
}

impl PostDom {
    /// Blocks that go nowhere leave through a virtual exit; a block that
    /// cannot reach one is in no tree.
    pub(super) fn of(
        succ: &[Vec<usize>],
        pred: &[Vec<usize>],
    ) -> Self {
        let n = succ.len();
        let exit = n;
        // Over the reversed graph from the virtual exit.
        let mut order: Vec<usize> = Vec::new();
        let mut seen = vec![false; n + 1];
        let mut stack: Vec<(usize, usize)> = vec![(exit, 0)];
        seen[exit] = true;
        let leaving: Vec<usize> = (0..n).filter(|&one| succ[one].is_empty()).collect();
        let onward = |at: usize, place: usize| -> Option<usize> {
            if at == exit { leaving.get(place).copied() } else { pred[at].get(place).copied() }
        };
        while let Some((at, place)) = stack.pop() {
            match onward(at, place) {
                Some(next) => {
                    stack.push((at, place + 1));
                    if !std::mem::replace(&mut seen[next], true) {
                        stack.push((next, 0));
                    }
                }
                None => order.push(at),
            }
        }
        order.reverse();
        let mut rank = vec![UNSET; n + 1];
        for (place, &at) in order.iter().enumerate() {
            rank[at] = place;
        }
        let mut idom = vec![UNSET; n + 1];
        idom[exit] = exit;
        let mut changed = true;
        while changed {
            changed = false;
            for &at in order.iter().skip(1) {
                let mut best = UNSET;
                let leaves = succ[at].is_empty().then_some(exit);
                for next in succ[at].iter().copied().chain(leaves) {
                    if idom[next] == UNSET {
                        continue;
                    }
                    best = if best == UNSET {
                        next
                    } else {
                        let (mut one, mut other) = (next, best);
                        while one != other {
                            while rank[one] > rank[other] {
                                one = idom[one];
                            }
                            while rank[other] > rank[one] {
                                other = idom[other];
                            }
                        }
                        one
                    };
                }
                if best != UNSET && idom[at] != best {
                    idom[at] = best;
                    changed = true;
                }
            }
        }
        let mut children: Vec<Vec<usize>> = vec![Vec::new(); n + 1];
        for &at in order.iter().skip(1) {
            if idom[at] != UNSET {
                children[idom[at]].push(at);
            }
        }
        let (mut enter, mut leave) = (vec![UNSET; n + 1], vec![UNSET; n + 1]);
        let (mut clock, mut walk) = (0, vec![(exit, 0)]);
        enter[exit] = 0;
        while let Some((at, place)) = walk.pop() {
            if let Some(&child) = children[at].get(place) {
                walk.push((at, place + 1));
                clock += 1;
                enter[child] = clock;
                walk.push((child, 0));
            } else {
                clock += 1;
                leave[at] = clock;
            }
        }
        enter.truncate(n);
        leave.truncate(n);
        Self { enter, leave }
    }

    /// `dominates(a, b)`: `a` is on every path from `b` to the exit. As LLVM's,
    /// a block outside the tree is post-dominated by any block and
    /// post-dominates none.
    pub(super) fn dominates(
        &self,
        a: usize,
        b: usize,
    ) -> bool {
        if a == b || self.enter[b] == UNSET {
            return true;
        }
        self.enter[a] != UNSET && self.enter[a] <= self.enter[b] && self.leave[b] <= self.leave[a]
    }
}

#[cfg(test)]
mod tests {
    use super::PostDom;

    /// 0 -> {1, 2}, 1 -> 3, 2 -> 3, 3 returns; 4 loops on itself.
    #[test]
    fn a_join_post_dominates_its_arms_and_a_loop_that_never_exits_is_outside_the_tree() {
        let succ = vec![vec![1, 2], vec![3], vec![3], vec![], vec![4]];
        let mut pred = vec![Vec::new(); 5];
        for (from, to) in succ.iter().enumerate() {
            for &one in to {
                pred[one].push(from);
            }
        }
        let tree = PostDom::of(&succ, &pred);
        assert!(tree.dominates(3, 0) && tree.dominates(3, 1) && tree.dominates(3, 2));
        assert!(!tree.dominates(1, 0) && !tree.dominates(2, 0) && !tree.dominates(0, 3));
        // Outside the tree: dominated by anything, dominating nothing.
        assert!(tree.dominates(3, 4) && !tree.dominates(4, 3));
    }
}

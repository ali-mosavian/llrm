//! The analyses a machine pass asks of a function's blocks: predecessors,
//! dominators, post-dominators and natural cycles (`MachineDominatorTree`,
//! `MachinePostDominatorTree`, `MachineCycleInfo`).
//!
//! DEVIATE: rebuilt from the function after each change instead of updated in
//! place, and a cycle is a natural loop (a back edge to a dominator); one
//! retreating edge to a non-dominator makes every cycle irreducible. Reason:
//! the spike keeps no analysis manager. Exit: the neutral-LIR task.

use crate::mfunc::MFunction;
use crate::regalloc::successors;

/// The blocks of a function, by index, and what is known of their order.
pub struct Cfg {
    pub succs: Vec<Vec<usize>>,
    pub preds: Vec<Vec<usize>>,
    idom: Vec<Option<usize>>,
    /// Post-dominator tree over the blocks and a virtual exit (the last index).
    ipdom: Vec<Option<usize>>,
    /// The header of each natural loop with its blocks, outermost first.
    loops: Vec<(usize, Vec<bool>)>,
    reducible: bool,
}

/// Cooper, Harvey and Kennedy's iterative immediate dominators over `count`
/// nodes from `root`.
fn immediate_dominators(
    count: usize,
    root: usize,
    succ: &[Vec<usize>],
    pred: &[Vec<usize>],
) -> Vec<Option<usize>> {
    let mut order = Vec::new();
    let mut seen = vec![false; count];
    let mut stack = vec![(root, 0usize)];
    seen[root] = true;
    while let Some((node, next)) = stack.pop() {
        if let Some(&child) = succ[node].get(next) {
            stack.push((node, next + 1));
            if !seen[child] {
                seen[child] = true;
                stack.push((child, 0));
            }
        } else {
            order.push(node);
        }
    }
    let mut number = vec![usize::MAX; count];
    for (at, node) in order.iter().enumerate() {
        number[*node] = at;
    }
    let mut idom: Vec<Option<usize>> = vec![None; count];
    idom[root] = Some(root);
    let intersect = |idom: &[Option<usize>], mut one: usize, mut two: usize| {
        while one != two {
            while number[one] < number[two] {
                one = idom[one].unwrap();
            }
            while number[two] < number[one] {
                two = idom[two].unwrap();
            }
        }
        one
    };
    let mut changed = true;
    while changed {
        changed = false;
        for &node in order.iter().rev() {
            if node == root {
                continue;
            }
            let mut best: Option<usize> = None;
            for &p in &pred[node] {
                if idom[p].is_none() {
                    continue;
                }
                best = Some(best.map_or(p, |current| intersect(&idom, p, current)));
            }
            if best != idom[node] {
                idom[node] = best;
                changed = true;
            }
        }
    }
    idom[root] = None;
    idom
}

impl Cfg {
    pub fn new<C>(function: &MFunction<C>) -> Cfg {
        let count = function.blocks.len();
        let mut succs = successors(function);
        for list in &mut succs {
            let mut seen = Vec::new();
            list.retain(|one| {
                let fresh = !seen.contains(one);
                seen.push(*one);
                fresh
            });
        }
        let mut preds = vec![Vec::new(); count];
        for (from, list) in succs.iter().enumerate() {
            for &to in list {
                preds[to].push(from);
            }
        }
        let idom = if count == 0 { Vec::new() } else { immediate_dominators(count, 0, &succs, &preds) };

        // the post-dominators: the graph reversed, from a virtual exit that
        // follows every block that leaves the function
        let exit = count;
        let mut rsucc = preds.clone();
        let mut rpred = succs.clone();
        rsucc.push(Vec::new());
        rpred.push(Vec::new());
        for block in 0..count {
            if succs[block].is_empty() {
                rsucc[exit].push(block);
                rpred[block].push(exit);
            }
        }
        let ipdom = immediate_dominators(count + 1, exit, &rsucc, &rpred);

        let mut cfg = Cfg { succs, preds, idom, ipdom, loops: Vec::new(), reducible: true };
        cfg.find_cycles();
        cfg
    }

    /// Natural loops from the back edges; a retreating edge to a block that
    /// does not dominate its source makes the function irreducible.
    fn find_cycles(&mut self) {
        let count = self.succs.len();
        let mut state = vec![0u8; count]; // 1 on the DFS stack, 2 done
        let mut stack = vec![(0usize, 0usize)];
        if count > 0 {
            state[0] = 1;
        }
        let mut back = Vec::new();
        while let Some((node, next)) = stack.pop() {
            if let Some(&child) = self.succs[node].get(next) {
                stack.push((node, next + 1));
                match state[child] {
                    0 => {
                        state[child] = 1;
                        stack.push((child, 0));
                    }
                    1 => back.push((node, child)),
                    _ => {}
                }
            } else {
                state[node] = 2;
            }
        }
        let mut loops: Vec<(usize, Vec<bool>)> = Vec::new();
        for (tail, header) in back {
            if !self.dominates(header, tail) {
                self.reducible = false;
                continue;
            }
            let at = match loops.iter().position(|(one, _)| *one == header) {
                Some(at) => at,
                None => {
                    let mut body = vec![false; count];
                    body[header] = true;
                    loops.push((header, body));
                    loops.len() - 1
                }
            };
            let mut work = vec![tail];
            while let Some(block) = work.pop() {
                if !loops[at].1[block] {
                    loops[at].1[block] = true;
                    work.extend(self.preds[block].iter().copied());
                }
            }
        }
        loops.sort_by_key(|(_, body)| std::cmp::Reverse(body.iter().filter(|b| **b).count()));
        self.loops = loops;
    }

    pub fn reachable(
        &self,
        block: usize,
    ) -> bool {
        block == 0 || self.idom[block].is_some()
    }

    /// `dominates`: every path from the entry to `lower` goes through `upper`.
    pub fn dominates(
        &self,
        upper: usize,
        lower: usize,
    ) -> bool {
        let mut at = lower;
        loop {
            if at == upper {
                return true;
            }
            match self.idom[at] {
                Some(up) => at = up,
                None => return false,
            }
        }
    }

    /// `PDT->dominates`: every path from `lower` out of the function goes
    /// through `upper`.
    pub fn post_dominates(
        &self,
        upper: usize,
        lower: usize,
    ) -> bool {
        let mut at = lower;
        loop {
            if at == upper {
                return true;
            }
            match self.ipdom[at] {
                Some(up) => at = up,
                None => return false,
            }
        }
    }

    /// The blocks whose immediate dominator is `block`.
    pub fn dominated_children(
        &self,
        block: usize,
    ) -> Vec<usize> {
        (0..self.succs.len()).filter(|other| self.idom[*other] == Some(block)).collect()
    }

    pub fn immediate_dominator(
        &self,
        block: usize,
    ) -> Option<usize> {
        self.idom[block]
    }

    /// `getCycleDepth`.
    pub fn cycle_depth(
        &self,
        block: usize,
    ) -> usize {
        self.loops.iter().filter(|(_, body)| body[block]).count()
    }

    /// The header of the innermost cycle holding `block` (`getCycle`).
    pub fn cycle(
        &self,
        block: usize,
    ) -> Option<usize> {
        self.loops.iter().rev().find(|(_, body)| body[block]).map(|(header, _)| *header)
    }

    /// `isReducible` of any cycle.
    pub fn reducible(&self) -> bool {
        self.reducible
    }
}

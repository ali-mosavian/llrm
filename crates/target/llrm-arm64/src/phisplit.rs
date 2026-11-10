//! Which critical edges phi elimination splits: `PHIElimination::SplitPHIEdges`
//! ([PHIElimination.cpp#L808](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PHIElimination.cpp#L808)).
//! The pin runs it with `LiveVariables`, which the optimizing register
//! allocation pipeline adds before the pass
//! ([TargetPassConfig.cpp#L1515](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TargetPassConfig.cpp#L1515));
//! the fast pipeline has none, so at -O0 it splits nothing.

use llrm_support::hash::{HashMap, HashSet};

use crate::mc::visit::Reg;
use crate::mfunc::{MFunc, Op};
use crate::prepass::{Cfg, natural_loops};

/// The edges `(from, to)` to split, by block position: one with several
/// successors that carries a phi's incoming value, where
/// - `from` is not `to` and not a back edge into a loop header from inside
///   the loop ([L829-L833](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PHIElimination.cpp#L829-L833)),
/// - the value is live out of `from` past the phis (`isLiveOutPastPHIs`, so
///   the copy would not be its kill; the early exit, `-no-phi-elim-live-out-early-exit`
///   is off), and
/// - either the value is not live into `to`, so another successor needs it
///   and the split avoids the interference, or the edge leaves a loop, or
///   enters a sibling loop's header
///   ([L853-L874](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PHIElimination.cpp#L853-L874)).
///
/// `phis[b]` are the phis of block `b` as `(incoming value, predecessor)`
/// lists; `successors` and `cfg` are those of the function before phi
/// elimination.
pub fn edges_to_split(
    function: &MFunc,
    cfg: &Cfg,
    phis: &[Vec<Vec<(Reg, usize)>>],
) -> HashSet<(usize, usize)> {
    let count = function.blocks.len();
    let live = Live::of(function, cfg, phis);
    let loops = natural_loops(function, cfg);
    // The innermost loop of a block (`MachineLoopInfo::getLoopFor`): the
    // smallest body that holds it.
    let innermost = |block: usize| -> Option<usize> {
        (0..loops.len()).filter(|&one| loops[one].1.contains(&block)).min_by_key(|&one| loops[one].1.len())
    };
    // `MachineLoop::contains(Inner)`.
    let contains = |outer: usize, inner: Option<usize>| -> bool {
        match inner {
            Some(inner) => outer == inner || loops[outer].1.contains(&loops[inner].0),
            None => false,
        }
    };
    let mut split = HashSet::default();
    for to in 0..count {
        let current = innermost(to);
        let header = current.is_some_and(|one| loops[one].0 == to);
        for phi in &phis[to] {
            for &(value, from) in phi {
                // One successor, a block named twice included, is no critical edge.
                let mut next = cfg.successors[from].clone();
                next.sort_unstable();
                next.dedup();
                if next.len() == 1 || split.contains(&(from, to)) {
                    continue;
                }
                if from == to || (header && innermost(from) == current) {
                    continue;
                }
                if !live.out_past_phis(value, from, cfg) {
                    continue;
                }
                let mut should = !live.is_live_in(value, to);
                let previous = innermost(from);
                if !should && current != previous {
                    should = previous.is_some_and(|one| !contains(one, current));
                }
                if should {
                    split.insert((from, to));
                }
            }
        }
    }
    split
}

/// Liveness of the virtual registers as `LiveVariables` has it: a phi's use
/// is at the end of its predecessor, so it makes the value live into that
/// block and not out of it.
struct Live {
    index: HashMap<Reg, usize>,
    words: usize,
    rows: Vec<u64>,
}

impl Live {
    fn of(
        function: &MFunc,
        cfg: &Cfg,
        phis: &[Vec<Vec<(Reg, usize)>>],
    ) -> Live {
        let count = function.blocks.len();
        let mut index: HashMap<Reg, usize> = HashMap::default();
        let mut number = |register: Reg| -> usize {
            let next = index.len();
            *index.entry(register).or_insert(next)
        };
        // Per block: what it reads before writing, and what it writes.
        let mut reads: Vec<Vec<usize>> = vec![Vec::new(); count];
        let mut writes: Vec<Vec<usize>> = vec![Vec::new(); count];
        for (at, block) in function.blocks.iter().enumerate() {
            let mut written: HashSet<usize> = HashSet::default();
            for op in &block.ops {
                if let Op::Phi { dst, .. } = op {
                    if dst.is_virtual() {
                        let dst = number(*dst);
                        written.insert(dst);
                        writes[at].push(dst);
                    }
                    continue;
                }
                let (defs, uses) = op.defs_and_uses();
                for register in uses.into_iter().filter(|register| register.is_virtual()) {
                    let register = number(register);
                    if !written.contains(&register) {
                        reads[at].push(register);
                    }
                }
                for register in defs.into_iter().filter(|register| register.is_virtual()) {
                    let register = number(register);
                    written.insert(register);
                    writes[at].push(register);
                }
            }
        }
        // A phi's use is at the end of its predecessor.
        for to in 0..count {
            for phi in &phis[to] {
                for &(value, from) in phi {
                    if value.is_virtual() {
                        let value = number(value);
                        if !writes[from].contains(&value) {
                            reads[from].push(value);
                        }
                    }
                }
            }
        }
        let words = index.len().div_ceil(64).max(1);
        let mut rows = vec![0u64; count * words];
        let set = |row: &mut [u64], bit: usize| row[bit / 64] |= 1 << (bit % 64);
        let mut changed = true;
        while changed {
            changed = false;
            for block in (0..count).rev() {
                let mut now = vec![0u64; words];
                for &next in &cfg.successors[block] {
                    for (word, bits) in now.iter_mut().zip(&rows[next * words..(next + 1) * words]) {
                        *word |= bits;
                    }
                }
                for &register in &writes[block] {
                    now[register / 64] &= !(1 << (register % 64));
                }
                for &register in &reads[block] {
                    set(&mut now, register);
                }
                let row = &mut rows[block * words..(block + 1) * words];
                if row != now.as_slice() {
                    row.copy_from_slice(&now);
                    changed = true;
                }
            }
        }
        Live { index, words, rows }
    }

    /// Whether `value` is live into `block` (`LiveVariables::isLiveIn`).
    fn is_live_in(
        &self,
        value: Reg,
        block: usize,
    ) -> bool {
        self.index.get(&value).is_some_and(|&bit| self.rows[block * self.words + bit / 64] >> (bit % 64) & 1 == 1)
    }

    /// Whether `value` is live out of `block` for some reason other than a
    /// phi use (`LiveVariables::isLiveOut`): live into a successor.
    fn out_past_phis(
        &self,
        value: Reg,
        block: usize,
        cfg: &Cfg,
    ) -> bool {
        cfg.successors[block].iter().any(|&next| self.is_live_in(value, next))
    }
}

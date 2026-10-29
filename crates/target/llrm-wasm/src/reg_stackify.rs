//! `WebAssemblyRegStackify` without optimization: a register whose definition
//! sits right before its only use moves onto the value stack, and the walk
//! continues through that definition's operands.
//!
//! # How it works
//! Instructions are visited last to first. Each one's register operands are
//! popped last operand first; one whose definition is the instruction just
//! above the current insertion point and which is read once is stackified, and
//! its definition's operands are pushed in turn. When an operand cannot be
//! stackified the instruction's commutable operands are swapped once to try
//! the other order.
//!
//! DEVIATE: only the `Optimize == false` path (the pin runs it at -O0): no
//! moving of definitions across other instructions, no rematerialization, no
//! tee for several uses, no dominance or live-interval queries, and one block.
//! Reason: those need `LiveIntervals`, the dominator tree and memory-effect
//! queries over instructions the slice lacks. At -O0 a move is always a no-op,
//! because the definition is already the instruction above. Exit: W8.
//!
//! # See also
//! `WebAssemblyRegStackify.cpp`, pin d1106deb71cc: `regStackify` (L841),
//! `isSafeToMove` (L296), `hasSingleUse` (L248), `TreeWalkerState` (L735),
//! `CommutingState` (L797).

use crate::dagisel::generated::instrinfo::{desc, flag};
use crate::mir::{Function, Inst, Operand, VReg, is_argument};

/// `TreeWalkerState`: ranges of the operands still to visit, last first.
struct TreeWalker {
    /// (instruction, one past the next operand to pop).
    worklist: Vec<(usize, usize)>,
}

impl TreeWalker {
    fn new(
        f: &Function,
        at: usize,
    ) -> TreeWalker {
        let mut walker = TreeWalker { worklist: Vec::new() };
        walker.push_operands(f, at);
        walker
    }

    fn defs(
        f: &Function,
        at: usize,
    ) -> usize {
        f.blocks[0][at].operands.iter().take_while(|o| matches!(o, Operand::Def(_))).count()
    }

    fn push_operands(
        &mut self,
        f: &Function,
        at: usize,
    ) {
        let end = f.blocks[0][at].operands.len();
        if end > Self::defs(f, at) {
            self.worklist.push((at, end));
        }
    }

    fn done(&self) -> bool {
        self.worklist.is_empty()
    }

    /// The next operand: its instruction and index.
    fn pop(
        &mut self,
        f: &Function,
    ) -> (usize, usize) {
        let (at, end) = self.worklist.pop().expect("a range remains");
        if end - 1 > Self::defs(f, at) {
            self.worklist.push((at, end - 1));
        }
        (at, end - 1)
    }

    fn has_remaining_operands(
        &self,
        at: usize,
    ) -> bool {
        self.worklist.last().is_some_and(|&(top, _)| top == at)
    }

    fn reset_top_operands(
        &mut self,
        f: &Function,
        at: usize,
    ) {
        *self.worklist.last_mut().expect("an operand remains") = (at, f.blocks[0][at].operands.len());
    }

    fn is_on_stack(
        &self,
        f: &Function,
        reg: VReg,
    ) -> bool {
        self.worklist.iter().any(|&(at, end)| f.blocks[0][at].operands[..end].contains(&Operand::Use(reg)))
    }
}

/// `CommutingState`.
#[derive(Default)]
struct Commuting {
    tentative: Option<(usize, usize)>,
    declined: bool,
}

impl Commuting {
    fn maybe_commute(
        &mut self,
        f: &mut Function,
        at: usize,
        walker: &mut TreeWalker,
    ) {
        if let Some((a, b)) = self.tentative.take() {
            f.blocks[0][at].operands.swap(a, b);
            self.declined = true;
        } else if !self.declined && walker.has_remaining_operands(at) {
            if let Some((a, b)) = commutable_operands(&f.blocks[0][at]) {
                f.blocks[0][at].operands.swap(a, b);
                walker.reset_top_operands(f, at);
                self.tentative = Some((a, b));
                self.declined = false;
            }
        }
    }

    fn reset(&mut self) {
        *self = Commuting::default();
    }
}

/// `findCommutedOpIndices`: the first two source registers of a commutable
/// instruction.
fn commutable_operands(inst: &Inst) -> Option<(usize, usize)> {
    if desc(inst.opcode)?.flags & flag::COMMUTABLE == 0 {
        return None;
    }
    let uses: Vec<usize> =
        inst.operands.iter().enumerate().filter(|(_, o)| matches!(o, Operand::Use(_))).map(|(i, _)| i).collect();
    (uses.len() >= 2).then(|| (uses[0], uses[1]))
}

/// Number of definitions and of uses of `reg`.
fn counts(
    f: &Function,
    reg: VReg,
) -> (usize, usize) {
    let (mut defs, mut uses) = (0, 0);
    for operand in f.blocks.iter().flatten().flat_map(|i| &i.operands) {
        match operand {
            Operand::Def(r) if *r == reg => defs += 1,
            Operand::Use(r) if *r == reg => uses += 1,
            _ => {}
        }
    }
    (defs, uses)
}

/// Whether anything was stackified.
pub fn reg_stackify(f: &mut Function) -> bool {
    assert_eq!(f.blocks.len(), 1, "one block until branches");
    let mut changed = false;
    let mut mii = f.blocks[0].len() as isize - 1;
    while mii >= 0 {
        let mut insert = mii as usize;
        let mut commuting = Commuting::default();
        let mut walker = TreeWalker::new(f, insert);
        while !walker.done() {
            let (at, index) = walker.pop(f);
            let Operand::Use(reg) = f.blocks[0][at].operands[index] else { continue };
            let (defs, uses) = counts(f, reg);
            let Some((_, def)) = f.vreg_def(reg).filter(|_| defs == 1) else { continue };
            if is_argument(f.blocks[0][def].opcode) {
                continue;
            }
            // `isSafeToMove` when not optimizing: one definition, directly
            // above.
            let explicit_defs = f.blocks[0][def].defs().count();
            let can_move = explicit_defs <= 1 && def + 1 == insert && !walker.is_on_stack(f, reg);
            if can_move && uses == 1 {
                f.stackify(reg);
                insert = def;
            } else {
                if !can_move {
                    commuting.maybe_commute(f, insert, &mut walker);
                }
                continue;
            }
            commuting.reset();
            walker.push_operands(f, insert);
        }
        if insert != mii as usize {
            mii = insert as isize;
            changed = true;
        }
        mii -= 1;
    }
    changed
}

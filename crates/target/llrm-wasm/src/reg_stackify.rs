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
//! tee for several uses, no dominance or live-interval queries.
//! Reason: those need `LiveIntervals`, the dominator tree and memory-effect
//! queries over instructions the slice lacks. At -O0 a move is always a no-op,
//! because the definition is already the instruction above. Exit: W8.
//!
//! # See also
//! `WebAssemblyRegStackify.cpp`, pin d1106deb71cc: `regStackify` (L841),
//! `isSafeToMove` (L296), `hasSingleUse` (L248), `TreeWalkerState` (L735),
//! `CommutingState` (L797).

use crate::dagisel::generated::ids::opcode as op;
use crate::dagisel::generated::instrinfo::{desc, flag};
use crate::mir::{DefUse, Function, IMPLICIT_DEF, Inst, Operand, VReg, ValType, is_argument};

/// `TreeWalkerState`: ranges of the operands still to visit, last first.
struct TreeWalker {
    /// The block the walk is in.
    b: usize,
    /// (instruction, one past the next operand to pop).
    worklist: Vec<(usize, usize)>,
}

impl TreeWalker {
    fn new(
        f: &Function,
        b: usize,
        at: usize,
    ) -> TreeWalker {
        let mut walker = TreeWalker { b, worklist: Vec::new() };
        walker.push_operands(f, at);
        walker
    }

    fn defs(
        &self,
        f: &Function,
        at: usize,
    ) -> usize {
        f.blocks[self.b][at].operands.iter().take_while(|o| matches!(o, Operand::Def(_))).count()
    }

    fn push_operands(
        &mut self,
        f: &Function,
        at: usize,
    ) {
        let end = f.blocks[self.b][at].operands.len();
        if end > self.defs(f, at) {
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
        if end - 1 > self.defs(f, at) {
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
        *self.worklist.last_mut().expect("an operand remains") = (at, f.blocks[self.b][at].operands.len());
    }

    fn is_on_stack(
        &self,
        f: &Function,
        reg: VReg,
    ) -> bool {
        self.worklist.iter().any(|&(at, end)| f.blocks[self.b][at].operands[..end].contains(&Operand::Use(reg)))
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
            f.blocks[walker.b][at].operands.swap(a, b);
            self.declined = true;
        } else if !self.declined && walker.has_remaining_operands(at) {
            if let Some((a, b)) = commutable_operands(&f.blocks[walker.b][at]) {
                f.blocks[walker.b][at].operands.swap(a, b);
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

/// Whether anything was stackified.
pub fn reg_stackify(f: &mut Function) -> bool {
    let mut changed = false;
    // Stackifying marks registers and commuting swaps operands; no instruction
    // is added or moved.
    let defuse = f.def_use();
    for b in 0..f.blocks.len() {
        changed |= stackify_block(f, &defuse, b);
    }
    changed
}

fn stackify_block(
    f: &mut Function,
    defuse: &DefUse,
    b: usize,
) -> bool {
    let mut changed = false;
    let mut mii = f.blocks[b].len() as isize - 1;
    while mii >= 0 {
        let mut insert = mii as usize;
        let mut commuting = Commuting::default();
        let mut walker = TreeWalker::new(f, b, insert);
        while !walker.done() {
            let (at, index) = walker.pop(f);
            let Operand::Use(reg) = f.blocks[b][at].operands[index] else { continue };
            let uses = defuse.use_count(reg);
            let Some((def_block, def)) = defuse.def(reg).filter(|_| defuse.def_count(reg) == 1) else { continue };
            if def_block != b || is_argument(f.blocks[b][def].opcode) {
                continue;
            }
            // `isSafeToMove` when not optimizing: one definition, directly
            // above.
            let explicit_defs = f.blocks[b][def].defs().count();
            let can_move = explicit_defs <= 1 && def + 1 == insert && !walker.is_on_stack(f, reg);
            if can_move && uses == 1 {
                f.stackify(reg);
                insert = def;
                // `convertImplicitDefToConstZero`: the push must be explicit.
                if f.blocks[b][def].opcode == IMPLICIT_DEF {
                    let (opcode, zero) = match f.vreg_type(reg) {
                        ValType::I32 => (op::CONST_I32, Operand::Imm(0)),
                        ValType::I64 => (op::CONST_I64, Operand::Imm(0)),
                        ValType::F32 => (op::CONST_F32, Operand::F32(0)),
                        ValType::F64 => (op::CONST_F64, Operand::F64(0)),
                    };
                    f.blocks[b][def] = Inst::new(opcode, &[Operand::Def(reg), zero]);
                }
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

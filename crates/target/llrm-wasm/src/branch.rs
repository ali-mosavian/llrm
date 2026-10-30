//! `WebAssemblyInstrInfo`'s branch hooks (`analyzeBranch`, `removeBranch`, `insertBranch`,
//! `reverseBranchCondition`) and `MachineBasicBlock::updateTerminator`, which CFGSort calls
//! after it moves a block.
//!
//! # See also
//! `WebAssemblyInstrInfo.cpp` L99..L200 and `MachineBasicBlock.cpp` (`updateTerminator`), pin
//! d1106deb71cc.

use crate::dagisel::generated::ids::opcode as op;
use crate::mir::{Inst, Operand, VReg};

/// `Cond`: whether the branch is taken when the register is true, and the register.
pub type Cond = Option<(bool, VReg)>;

/// What `analyzeBranch` finds.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Branches {
    pub taken: Option<usize>,
    pub fall: Option<usize>,
    pub cond: Cond,
}

/// The terminators of a block: the run of terminators at its end.
pub fn terminators(block: &[Inst]) -> &[Inst] {
    let start = block.iter().rposition(|inst| !inst.is_terminator()).map_or(0, |last| last + 1);
    &block[start..]
}

fn target(inst: &Inst) -> usize {
    let Operand::Block(block) = inst.operands[0] else { unreachable!("a branch names its block first") };
    block as usize
}

/// `analyzeBranch`; `None` where it returns true (a terminator it cannot model).
pub fn analyze(block: &[Inst]) -> Option<Branches> {
    let mut found = Branches::default();
    let mut have_cond = false;
    for inst in terminators(block) {
        match inst.opcode {
            op::BR_IF | op::BR_UNLESS => {
                if have_cond {
                    return None;
                }
                let Operand::Use(reg) = inst.operands[1] else { unreachable!("a conditional branch reads a register") };
                found.cond = Some((inst.opcode == op::BR_IF, reg));
                found.taken = Some(target(inst));
                have_cond = true;
            }
            op::BR => {
                if have_cond {
                    found.fall = Some(target(inst));
                } else {
                    found.taken = Some(target(inst));
                }
            }
            _ => return None,
        }
        if inst.is_barrier() {
            break;
        }
    }
    Some(found)
}

/// `removeBranch`: the terminators, all of them; how many.
pub fn remove(block: &mut Vec<Inst>) -> usize {
    let start = block.len() - terminators(block).len();
    let count = block.len() - start;
    block.truncate(start);
    count
}

/// `insertBranch`.
pub fn insert(
    block: &mut Vec<Inst>,
    taken: Option<usize>,
    fall: Option<usize>,
    cond: Cond,
) {
    match cond {
        None => {
            if let Some(taken) = taken {
                block.push(Inst::new(op::BR, &[Operand::Block(taken as u32)]));
            }
        }
        Some((when, reg)) => {
            let opcode = if when { op::BR_IF } else { op::BR_UNLESS };
            let taken = taken.expect("a conditional branch has a target");
            block.push(Inst::new(opcode, &[Operand::Block(taken as u32), Operand::Use(reg)]));
            if let Some(fall) = fall {
                block.push(Inst::new(op::BR, &[Operand::Block(fall as u32)]));
            }
        }
    }
}

/// `updateTerminator`: `block` is followed in the layout by `next`; `previous` is the block that
/// followed it before, whose edge a fallthrough is.
pub fn update_terminator(
    code: &mut Vec<Inst>,
    succs: &[usize],
    next: Option<usize>,
    previous: Option<usize>,
) {
    if succs.is_empty() {
        return;
    }
    let Some(found) = analyze(code) else { unreachable!("UpdateTerminators requires analyzable predecessors") };
    let Branches { taken, fall, cond } = found;
    let is_next = |block: Option<usize>| block.is_some() && block == next;
    let Some((when, reg)) = cond else {
        if let Some(taken) = taken {
            if is_next(Some(taken)) {
                remove(code);
            }
        } else {
            let Some(previous) = previous.filter(|previous| succs.contains(previous)) else { return };
            if !is_next(Some(previous)) {
                insert(code, Some(previous), None, None);
            }
        }
        return;
    };
    if let Some(fall) = fall {
        if is_next(taken) {
            remove(code);
            insert(code, Some(fall), None, Some((!when, reg)));
        } else if is_next(Some(fall)) {
            remove(code);
            insert(code, taken, None, Some((when, reg)));
        }
        return;
    }
    let previous = previous.expect("a fallthrough conditional branch has a layout successor");
    if Some(previous) == taken {
        remove(code);
        if !is_next(taken) {
            insert(code, taken, None, None);
        }
        return;
    }
    if is_next(taken) {
        remove(code);
        insert(code, Some(previous), None, Some((!when, reg)));
    } else if !is_next(Some(previous)) {
        remove(code);
        insert(code, taken, Some(previous), Some((when, reg)));
    }
}

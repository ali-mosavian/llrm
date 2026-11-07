//! A store is moved down to the store of the neighbouring address that follows
//! it, and a load up to the load of the neighbouring address that precedes it,
//! so that the two are adjacent for `pairs` (the pin's MachineScheduler
//! clusters such accesses: `AArch64InstrInfo::shouldClusterMemOps`).
//!
//! # How it works
//! Machine form, virtual registers, before allocation. A store to `[base,
//! #imm]` looks a few instructions ahead for a store of the same size to the
//! next or previous slot from the same base. It passes instructions that
//! neither redefine the base or the value it stores, nor touch memory it does
//! not commute with, and is placed right before the second. Moved before
//! allocation, it costs nothing in registers the way moving it afterwards
//! would: the value lives to the second store, instead of the allocator
//! reusing its register for the value computed between them.
//!
//! ```text
//!   x = a*3 ; str x,[p] ; y = a*5 ; str y,[p,#8]
//!   x = a*3 ; y = a*5 ; str x,[p] ; str y,[p,#8]      -> stp x, y, [p]
//!
//!   x = ldr [p] ; s = f(x) ; y = ldr [p,#8] ; t = g(y)
//!   x = ldr [p] ; y = ldr [p,#8] ; s = f(x) ; t = g(y) -> ldp x, y, [p]
//! ```
//! A load is hoisted, not its predecessor sunk: the first one's value is read
//! in between. The hoisted one defines a fresh register, so nothing between
//! reads it.

use crate::mc::inst::*;
use crate::mc::visit::Reg;
use crate::mfunc::{MFunc, Op, OpList};
use crate::pairs::{Access, access, commutes, leaves_the_block, pair_of, touch, touch_of};

/// How many instructions ahead a store looks.
const WINDOW: usize = 12;

fn cluster(mut ops: OpList) -> OpList {
    let mut i = 0;
    while i < ops.len() {
        let Op::I(inst) = &ops[i] else {
            i += 1;
            continue;
        };
        let Some(first) = access_of(inst).filter(|first| !first.load) else {
            i += 1;
            continue;
        };
        let (float, number) = crate::pairs::register(&first.mem);
        let source = if float { Reg::Fpr(Vreg(number)) } else { Reg::Gpr(Gpr(number)) };
        let base = Reg::Gpr(first.base);
        for j in i + 1..(i + 1 + WINDOW).min(ops.len()) {
            if let Op::I(other) = &ops[j]
                && let Some(second) = access_of(other).filter(|second| !second.load)
                && pair_of(&first, &second).is_some()
            {
                if j > i + 1 {
                    let (moved, loc) = ops.remove(i);
                    ops.insert_at(j - 1, moved, loc);
                }
                break;
            }
            let (defs, _) = ops[j].defs_and_uses();
            let passable = match &ops[j] {
                Op::I(between) => {
                    !leaves_the_block(between)
                        && (!crate::pairs::is_memory(between)
                            || touch(between).is_some_and(|other| commutes(&touch_of(&first), &other)))
                }
                Op::Copy { .. } | Op::Imm { .. } | Op::FrameAddress { .. } | Op::DbgValue { .. } => true,
                Op::Call { .. } | Op::Return { .. } | Op::JumpTable { .. } => false,
            };
            if !passable || defs.contains(&base) || defs.contains(&source) {
                break;
            }
        }
        i += 1;
    }
    ops
}

/// `access` of a load or store, where a frame slot is the base: its address
/// is `slot + offset`, and no instruction changes it, so a stand-in register
/// that nothing defines names the slot.
fn access_of(inst: &Inst) -> Option<Access> {
    let (Inst::Load { mem, addr: Addr::Slot { slot, offset }, mmo } | Inst::Store { mem, addr: Addr::Slot { slot, offset }, mmo }) =
        inst
    else {
        return access(inst);
    };
    let addr = Addr::Offset { base: Gpr(SLOT_BASE | slot), imm: *offset };
    let (mem, mmo) = (*mem, mmo.clone());
    access(&if matches!(inst, Inst::Load { .. }) { Inst::Load { mem, addr, mmo } } else { Inst::Store { mem, addr, mmo } })
}

/// Register numbers of the stand-ins for frame slots: above any virtual one.
const SLOT_BASE: u32 = 1 << 31;

/// Whether `ops[between]` may be crossed by the access `moved`, which
/// changes places with it: it does not define the base, touch the register
/// `value` the access loads, leave the block or order against `moved` in
/// memory.
fn passes(
    ops: &OpList,
    between: std::ops::Range<usize>,
    base: Reg,
    moved: &Access,
    value: Reg,
) -> bool {
    between.into_iter().all(|k| {
        let (defs, uses) = ops[k].defs_and_uses();
        let passable = match &ops[k] {
            Op::I(inst) => {
                !leaves_the_block(inst)
                    && (!crate::pairs::is_memory(inst)
                        || touch(inst).is_some_and(|other| commutes(&touch_of(moved), &other)))
            }
            Op::Copy { .. } | Op::Imm { .. } | Op::FrameAddress { .. } | Op::DbgValue { .. } => true,
            Op::Call { .. } | Op::Return { .. } | Op::JumpTable { .. } => false,
        };
        passable && !defs.contains(&base) && !defs.contains(&value) && !uses.contains(&value)
    })
}

/// A load moves up to right after the load of the neighbouring slot from the
/// same base that precedes it.
fn cluster_loads(mut ops: OpList) -> OpList {
    let mut i = 0;
    while i < ops.len() {
        let Op::I(inst) = &ops[i] else {
            i += 1;
            continue;
        };
        let Some(first) = access_of(inst).filter(|first| first.load) else {
            i += 1;
            continue;
        };
        let base = Reg::Gpr(first.base);
        for j in i + 2..(i + 1 + WINDOW).min(ops.len()) {
            if let Op::I(other) = &ops[j]
                && let Some(second) = access_of(other).filter(|second| second.load)
                && pair_of(&first, &second).is_some()
            {
                let (float, number) = crate::pairs::register(&second.mem);
                let value = if float { Reg::Fpr(Vreg(number)) } else { Reg::Gpr(Gpr(number)) };
                if passes(&ops, i + 1..j, base, &second, value) {
                    let (moved, loc) = ops.remove(j);
                    ops.insert_at(i + 1, moved, loc);
                }
                break;
            }
        }
        i += 1;
    }
    ops
}

pub fn run(function: &mut MFunc) {
    for block in &mut function.blocks {
        block.ops = cluster_loads(cluster(std::mem::take(&mut block.ops)));
    }
}

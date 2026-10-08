//! LLVM's `hasFP`, decided before allocation: whether a function can do without its frame register, which is then one more value
//! register (and callee-saved: the prologue pushes it where it is used).
//!
//! It can where `masm` can address every frame cell through the stack pointer, which needs the stack's depth at every instruction.
//! This reads the same things `masm::stack_addressed` reads, from the LIR before allocation: spill code adds cells and moves, no
//! push or pop. What cannot be known here (a stack pointer written another way, an indirect call that may pop, inline code, a frame
//! register read as a value) keeps the frame register.

use std::collections::{BTreeMap, BTreeSet};

use iced_x86::Register;

use crate::model::ir::{Loc, Operation};
use crate::model::lir::LirBody;

/// The registers that are the frame register as LIR names it.
fn is_frame(register: Register, pointer: Register) -> bool {
    register != Register::None && crate::model::ir::root(register) == crate::model::ir::root(pointer)
}

/// Whether the function described by `body` (its calls' popped bytes in `pops`) can have no frame register.
pub fn without_frame_register(body: &LirBody, registers: &llrm_target::FrameRegisters, pops: &BTreeMap<i64, i64>, inline: bool, far: bool, landing: bool) -> bool {
    let pointer = registers.pointer;
    if !registers.optional || body.bits != 32 || far || inline || landing || body.returns_twice || (!body.variables.is_empty() && !body.cfa_variables) {
        return false;
    }
    // The depth at the top of each block, and each block's effect.
    let mut effects: BTreeMap<i64, Option<i64>> = BTreeMap::new();
    // The depth within a block at which it returns.
    let mut leaves: BTreeMap<i64, i64> = BTreeMap::new();
    for block in &body.blocks {
        let mut depth = 0i64;
        for one in &block.insns {
            let Some(what) = &one.what else { continue };
            for place in what.dests.iter().chain(&what.sources) {
                match place {
                    Loc::Reg(reg) if is_frame(reg.register, pointer) => return false,
                    Loc::Mem(cell) if is_frame(cell.index_through, pointer) => return false,
                    Loc::Address(address) if is_frame(address.index, pointer) => return false,
                    // A register-based cell is fine; the frame register as a base is only a frame cell.
                    Loc::Mem(cell) if is_frame(cell.through, pointer) && !cell.in_frame() => return false,
                    Loc::Address(address) if is_frame(address.through, pointer) && address.addr.as_ref().is_none_or(|addr| addr.space != crate::model::ir::Space::Frame && addr.space != crate::model::ir::Space::Literal) => return false,
                    _ => {}
                }
            }
            match what.op {
                Operation::Push | Operation::Pop => {
                    let (places, sign) = if what.op == Operation::Push { (&what.sources, 1) } else { (&what.dests, -1) };
                    let [place] = &places[..] else { return false };
                    let width = match place {
                        Loc::Reg(reg) => reg.register.size() as i64,
                        Loc::Imm(imm) => i64::from(imm.width),
                        Loc::Mem(cell) => i64::from(cell.width),
                        _ => return false,
                    };
                    if width != 2 && width != 4 {
                        return false;
                    }
                    depth += sign * width;
                }
                Operation::Leave | Operation::Escape => return false,
                Operation::Nothing if what.name.as_deref().is_some_and(|name| name.starts_with("push") || name.starts_with("pop")) => return false,
                Operation::Call => {
                    if what.indirect {
                        return false;
                    }
                    depth -= pops.get(&one.at).copied().unwrap_or(0);
                }
                Operation::Jump | Operation::Branch if what.indirect => return false,
                Operation::Return => leaves.insert(block.at, depth).map_or((), |_| ()),
                _ if what.dests.iter().any(|place| matches!(place, Loc::Reg(reg) if crate::model::ir::root(reg.register) == crate::model::ir::root(registers.stack))) => {
                    // Only `add sp, n` and `sub sp, n` are understood.
                    let amount = match (&what.dests[..], &what.sources[..]) {
                        ([Loc::Reg(dest)], [Loc::Reg(source), Loc::Imm(imm)]) if dest.register == source.register && imm.address.is_none() => imm.value,
                        _ => return false,
                    };
                    match what.name.as_deref() {
                        Some("add") => depth -= amount,
                        Some("sub") => depth += amount,
                        _ => return false,
                    }
                }
                _ => {}
            }
        }
        effects.insert(block.at, Some(depth));
    }
    // Every way into a block agrees on the depth.
    let mut at: BTreeMap<i64, i64> = BTreeMap::new();
    let mut pending = vec![(body.entry, 0i64)];
    let mut seen = BTreeSet::new();
    while let Some((block, here)) = pending.pop() {
        match at.get(&block) {
            Some(&there) if there != here => return false,
            Some(_) => continue,
            None => {
                at.insert(block, here);
            }
        }
        seen.insert(block);
        if leaves.get(&block).is_some_and(|within| here + within != 0) {
            return false;
        }
        let Some(Some(effect)) = effects.get(&block) else { continue };
        let Some(found) = body.blocks.iter().find(|one| one.at == block) else { continue };
        for &next in &found.succ {
            pending.push((next, here + effect));
        }
    }
    llrm_support::debug!("frame", "{}: freed", body.name);
    true
}

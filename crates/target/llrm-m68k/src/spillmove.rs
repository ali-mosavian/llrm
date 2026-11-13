//! A spill and a reload as `move.l`: `move.l %d0, (d,%sp)` is 4 B where the
//! pin's `movem.l %d0, (d,%sp)` is 6, and no slower.
//!
//! # How it works
//! After allocation, [`spills_as_moves`] turns each `MOVM32pm_P` and
//! `MOVM32mp_P` into `MOV32pr` and `MOV32rp` where the codes are dead after it:
//! the next instruction of the block that touches them sets them, or none does
//! and no block it reaches reads them first. A reload into an address register
//! (`movea.l`) leaves the codes alone and always turns. Both forms are priced
//! on the cpu, and the `move` kept only where it wins.
//!
//! DEVIATE: the pin spills with `MOVEM` whatever the codes
//! (`M68kInstrInfo::getStoreRegOpcode`: a `move` would overwrite the codes a
//! branch may still read). Reason: best of gcc and clang is the goal; gcc
//! spills with `move.l`, and the codes are dead at most spills. Owner: m68k.
//! Exit: 2026-12-12, the shared flow chooses the spill opcode by the codes'
//! liveness.
//!
//! # See also
//! `M68kInstrInfo::storeRegToStackSlot`, [`crate::flags`].

use llrm_spike_mf::regalloc::successors;
use llrm_spike_mf::{Inst, Kind, Operand};

use crate::dagisel::hooks::Cpu;
use crate::mfunc::MFunction;
use crate::regalloc::Assignment;
use crate::timing::Price;
use crate::{REGISTER_INFO, mc, reg};

/// Whether `inst` reads or sets the codes, first: `Some(true)` reads,
/// `Some(false)` sets without reading, `None` neither. A call sets them (it
/// leaves them undefined).
fn touches(inst: &Inst) -> Option<bool> {
    if inst.reads_flags() && !inst.call {
        Some(true)
    } else if inst.sets_flags() || inst.call {
        Some(false)
    } else {
        None
    }
}

/// For each block, whether the codes are read on entry before anything sets
/// them.
fn live_in(function: &MFunction) -> Vec<bool> {
    let successors = successors(function);
    let mut live = vec![false; function.blocks.len()];
    loop {
        let mut changed = false;
        for (at, block) in function.blocks.iter().enumerate().rev() {
            let now = match block.insts.iter().find_map(touches) {
                Some(reads) => reads,
                None => successors[at].iter().any(|&next| live[next]),
            };
            if now && !live[at] {
                live[at] = true;
                changed = true;
            }
        }
        if !changed {
            return live;
        }
    }
}

/// What `opcode` over `operands` costs on `cpu`: bytes and clocks.
fn price_of(
    opcode: &'static str,
    operands: Vec<mc::Operand>,
    cpu: Cpu,
) -> Option<(usize, u32)> {
    let one = mc::Inst { opcode, operands };
    let bytes = crate::encode::encode(&one).ok()?.bytes.len();
    let clocks = crate::timing::cost(cpu, &mc::print(&one).ok()?, bytes as u32, false).ok()?.clocks;
    Some((bytes, clocks))
}

/// Spills and reloads as `move.l` where the codes are dead, as described above.
pub fn spills_as_moves(
    function: &mut MFunction,
    assignment: &Assignment,
    cpu: Cpu,
    price: Price,
) {
    use mc::Operand::{Imm, Reg};
    let store = (
        price_of("MOV32pr", vec![Imm(8), Reg(reg::SP), Reg(reg::D0)], cpu),
        price_of("MOVM32pm", vec![Imm(8), Reg(reg::SP), Imm(1)], cpu),
    );
    let load = (
        price_of("MOV32rp", vec![Reg(reg::D0), Imm(8), Reg(reg::SP)], cpu),
        price_of("MOVM32mp", vec![Imm(1), Imm(8), Reg(reg::SP)], cpu),
    );
    let wins = |(moved, packed): (Option<(usize, u32)>, Option<(usize, u32)>)| match (moved, packed) {
        (Some(moved), Some(packed)) => price.beats(cpu, moved, packed),
        _ => false,
    };
    let (store, load) = (wins(store), wins(load));
    if !store && !load {
        return;
    }
    let live = live_in(function);
    let successors = successors(function);
    for at in 0..function.blocks.len() {
        for i in 0..function.blocks[at].insts.len() {
            let inst = &function.blocks[at].insts[i];
            let (is_store, is_load) = (inst.kind == Kind::Op("MOVM32pm_P"), inst.kind == Kind::Op("MOVM32mp_P"));
            if !(is_store && store || is_load && load) {
                continue;
            }
            // `movea.l` leaves the codes alone
            let to_address = is_load
                && matches!(
                    inst.operands.first(),
                    Some(Operand::Def(r)) if { let p = match r { llrm_spike_mf::Reg::Virt(v) => assignment.0[*v as usize], llrm_spike_mf::Reg::Phys(p) => *p, }; crate::expand::is_address(REGISTER_INFO.root(p)) }
                );
            let dead = to_address
                || match function.blocks[at].insts[i + 1..].iter().find_map(touches) {
                    Some(reads) => !reads,
                    None => !successors[at].iter().any(|&next| live[next]),
                };
            if !dead {
                continue;
            }
            let inst = &mut function.blocks[at].insts[i];
            inst.kind = Kind::Op(if is_store { "MOV32pr" } else { "MOV32rp" });
            inst.sets_flags = !to_address;
        }
    }
}

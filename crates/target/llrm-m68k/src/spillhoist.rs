//! A spill or reload between a compare and its branch goes above the compare:
//! `cmp.b %d5,%d3; movem.l (8,%sp),%d0; bhi` is `move.l (8,%sp),%d0; cmp.b %d5,%d3; bhi`.
//!
//! # How it works
//! After allocation a spill access that sits between a codes setter and the
//! instruction that reads them must keep the codes: it stays the 6 B `movem`
//! pseudo ([`crate::spillmove`]), and its slot cannot move into an address
//! register ([`crate::slotreg`]). [`hoist_spills`] moves such an access above
//! the nearest setter, where that setter overwrites what the `move` would
//! set, when nothing from the setter up to the access names the register
//! (a reload) or defines it (a store), touches the slot, or calls.
//!
//! DEVIATE: the pin spills with `MOVEM` whatever the codes
//! (`M68kInstrInfo::getStoreRegOpcode`), so it has nothing to hoist past.
//! Reason: best of gcc and clang is the goal; gcc spills with `move.l` and
//! keeps its spills off the compare-to-branch path. Owner: m68k. Exit: none.
//! Off for `Options::PINNED`.
//!
//! # See also
//! `MachineScheduler`'s post-RA placement, [`crate::spillmove`].

use llrm_spike_mf::{Inst, Kind, Operand, Reg};

use crate::REGISTER_INFO;
use crate::mfunc::MFunction;
use crate::regalloc::Assignment;
use crate::spillmove::touches;

/// Moves each blocked spill access above its codes setter; returns how many moved.
pub fn hoist_spills(
    function: &mut MFunction,
    assignment: &Assignment,
) -> usize {
    let root = |register: Reg| match register {
        Reg::Virt(v) => REGISTER_INFO.root(assignment.0[v as usize]),
        Reg::Phys(p) => REGISTER_INFO.root(p),
    };
    let mut moved = 0;
    for block in &mut function.blocks {
        let mut i = 0;
        while i < block.insts.len() {
            let (load, store) = (block.insts[i].kind == Kind::Op("MOVM32mp_P"), block.insts[i].kind == Kind::Op("MOVM32pm_P"));
            if (load || store) && next_touch_reads(&block.insts[i + 1..]) && let Some(at) = hoist_target(&block.insts, i, load, &root) {
                let access = block.insts.remove(i);
                block.insts.insert(at, access);
                moved += 1;
            }
            i += 1;
        }
    }
    moved
}

/// Whether the first instruction that touches the codes reads them.
fn next_touch_reads(rest: &[Inst]) -> bool {
    rest.iter().find_map(touches) == Some(true)
}

/// Where the access at `i` goes: the nearest earlier setter that overwrites the codes without reading them, when
/// nothing from there up to `i` conflicts with the access.
fn hoist_target(
    insts: &[Inst],
    i: usize,
    load: bool,
    root: &impl Fn(Reg) -> llrm_lir::registers::RegId,
) -> Option<usize> {
    let register = insts[i].operands.iter().find_map(|operand| match operand {
        Operand::Def(r) | Operand::Use(r) => Some(root(*r)),
        _ => None,
    })?;
    let slot = insts[i].operands.iter().find_map(|operand| if let Operand::Frame(s) = operand { Some(*s) } else { None })?;
    for at in (0..i).rev() {
        let inst = &insts[at];
        let conflict = inst.call
            || inst.operands.iter().any(|operand| match operand {
                Operand::Frame(s) => *s == slot,
                // a reload's register may not appear; a store's may not be written
                Operand::Def(r) => root(*r) == register,
                Operand::Use(r) => load && root(*r) == register,
                _ => false,
            });
        if conflict {
            return None;
        }
        if touches(inst) == Some(false) {
            return Some(at);
        }
        if touches(inst) == Some(true) {
            // a reader between: the codes it reads come from before it
            return None;
        }
    }
    None
}

//! `cmpa.l`: a compare of a value that lives in an address register.
//!
//! # How it works
//! `cmp.l` takes a data register as its destination, so a pointer in `%a0`
//! compared with another value was copied to a data register first:
//! `move.l %a0, %d1; cmp.l %a1, %d1`. [`compare_address_registers`] runs after
//! allocation and, where the copy's register is read by nothing but the
//! compare, compares the address register itself (`cmpa.l %a1, %a0`) and drops
//! the copy: 2 bytes and 4 clocks on the 68000 each time. The compare sets the
//! same codes either way (a long compare of the whole register).
//!
//! A value only a compare's destination keeps in a data register may live in
//! an address register: [`widen_compare_destinations`] gives it the class
//! `XR32` before allocation, [`compare_in_address_register`] makes the `cmpa.l`
//! after it, and [`word_immediates`] makes `cmpa.l #k` of a 16-bit `k` the 4 B
//! `cmpa.w #k`. A function it changed is also built without, the cheaper kept
//! (`Options::compare_destinations`, PARITY row 48).
//!
//! DEVIATE C-CMPA: the pin's M68k has no `CMPA` row (see
//! [`crate::dagisel::cmpa`]) and gives a compare's destination `DR32`. Owner
//! m68k; exit 2026-11-28, with the rows.

use llrm_spike_mf::livephys::PhysLiveness;
use llrm_spike_mf::{Kind, Operand, Reg};

use crate::dagisel::{cmpa, local};
use crate::mfunc::{Class, MFunction, op_inst, operand_class};
use crate::regalloc::{Assignment, M68kRegisters};
use crate::{REGISTER_INFO, reg};

/// The destination of a long compare may live in an address register: a
/// register whose other operands all allow one takes the class `XR32`, and the
/// allocator chooses; how many were widened. [`compare_in_address_register`]
/// makes the `cmpa.l` after allocation.
pub fn widen_compare_destinations(function: &mut MFunction) -> usize {
    let count = function.classes.len();
    let mut destination = vec![false; count];
    let mut blocked = vec![false; count];
    let mut note = |register: &Reg, ok: bool, dest: bool| {
        if let Reg::Virt(v) = register {
            destination[*v as usize] |= dest;
            blocked[*v as usize] |= !ok;
        }
    };
    for block in &function.blocks {
        for phi in &block.phis {
            note(&phi.result, true, false);
            for (_, input) in &phi.incoming {
                note(input, true, false);
            }
        }
        for inst in &block.insts {
            for (position, operand) in inst.operands.iter().enumerate() {
                let (Operand::Def(r) | Operand::Use(r)) = operand else { continue };
                match inst.kind {
                    Kind::Copy => note(r, true, false),
                    Kind::Op(name) if cmpa::dest_of(name) == Some(position) => note(r, true, true),
                    Kind::Op(name) => note(r, operand_class(name, position) == Some(Class::Xr32), false),
                }
            }
        }
    }
    let mut widened = 0;
    for (v, class) in function.classes.iter_mut().enumerate() {
        if *class == Class::Dr32 && destination[v] && !blocked[v] {
            *class = Class::Xr32;
            widened += 1;
        }
    }
    widened
}

/// Rewrites each compare whose destination the allocation put in an address
/// register to the `cmpa.l` of it.
pub fn compare_in_address_register(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    let address = [reg::A0, reg::A1, reg::A2, reg::A3, reg::A4, reg::A5];
    for inst in function.blocks.iter_mut().flat_map(|block| &mut block.insts) {
        let Kind::Op(name) = inst.kind else { continue };
        let (Some(sibling), Some(dest_at)) = (cmpa::sibling(name), cmpa::dest_of(name)) else { continue };
        let Some(&Operand::Use(dest)) = inst.operands.get(dest_at) else { continue };
        let physical = match dest {
            Reg::Virt(v) => assignment.0[v as usize],
            Reg::Phys(p) => p,
        };
        if !address.contains(&REGISTER_INFO.root(physical)) {
            continue;
        }
        let Some(desc) = local::code_of(sibling).and_then(local::desc) else { continue };
        *inst = op_inst(sibling, desc, inst.operands.clone());
    }
}

/// `cmpa.l #imm, %an` with `imm` in 16 bits is `cmpa.w #imm, %an`: the word
/// is sign-extended and the whole register compared. 4 bytes and 10 clocks on
/// the 68000 for 6 and 14, with no register to hold the constant.
pub fn word_immediates(function: &mut MFunction) {
    for inst in function.blocks.iter_mut().flat_map(|block| &mut block.insts) {
        if inst.kind != Kind::Op("CMPA32i") {
            continue;
        }
        let [Operand::Imm(value), Operand::Use(_)] = &inst.operands[..] else { continue };
        if !(-32768..=32767).contains(value) {
            continue;
        }
        let Some(desc) = local::code_of("CMPA16i").and_then(local::desc) else { continue };
        *inst = op_inst("CMPA16i", desc, inst.operands.clone());
    }
}

/// Rewrites each copy to a data register and the compare of it, as described
/// above.
pub fn compare_address_registers(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    // `LLRM_NO_CMPA` turns it off, for an A/B of one build.
    if llrm_support::env_flag!("LLRM_NO_CMPA") {
        return;
    }
    let data = [reg::D0, reg::D1, reg::D2, reg::D3, reg::D4, reg::D5, reg::D6, reg::D7];
    let address = [reg::A0, reg::A1, reg::A2, reg::A3, reg::A4, reg::A5];
    let root = |register: Reg| match register {
        Reg::Virt(v) => REGISTER_INFO.root(assignment.0[v as usize]),
        Reg::Phys(p) => REGISTER_INFO.root(p),
    };
    let liveness = PhysLiveness::of::<M68kRegisters>(function, assignment, &REGISTER_INFO);
    for b in 0..function.blocks.len() {
        for at in (0..function.blocks[b].insts.len()).rev() {
            let inst = &function.blocks[b].insts[at];
            let Kind::Op(name) = inst.kind else { continue };
            let (Some(sibling), Some(dest_at)) = (cmpa::sibling(name), cmpa::dest_of(name)) else { continue };
            let Some(&Operand::Use(dest)) = inst.operands.get(dest_at) else { continue };
            if !data.contains(&root(dest)) {
                continue;
            }
            // the compare's other operands may not name the register
            let others = inst
                .operands
                .iter()
                .enumerate()
                .any(
                    |(index, operand)| index != dest_at
                        && matches!(
                            operand,
                            Operand::Def(r) | Operand::Use(r) if root(*r) == root(dest)
                        ),
                );
            if others {
                continue;
            }
            // the copy that made it, with nothing between that touches either
            // register
            let mut found = None;
            for earlier in (0..at).rev() {
                let candidate = &function.blocks[b].insts[earlier];
                if let (Kind::Copy, [Operand::Def(to), Operand::Use(from)]) = (candidate.kind, &candidate.operands[..])
                    && root(*to) == root(dest)
                {
                    if address.contains(&root(*from)) {
                        found = Some((earlier, *from));
                    }
                    break;
                }
                let touches = candidate
                    .operands
                    .iter()
                    .any(
                        |operand| match operand {
                            Operand::Def(r) | Operand::Use(r) => root(*r) == root(dest),
                            _ => false,
                        },
                    );
                if touches || candidate.call || candidate.terminator {
                    break;
                }
            }
            let Some((copy_at, from)) = found else { continue };
            let between_writes = function
                .blocks[b]
                .insts[copy_at + 1..at]
                .iter()
                .any(
                    |candidate| candidate.call
                        || candidate
                            .operands
                            .iter()
                            .any(|operand| matches!(operand, Operand::Def(r) if root(*r) == root(from))),
                );
            if between_writes {
                continue;
            }
            // the data register is dead after the compare
            let after = liveness.live_before::<M68kRegisters>(function, assignment, &REGISTER_INFO, b, at + 1);
            if after.is_live(&REGISTER_INFO, root(dest)) {
                continue;
            }
            let Some(desc) = local::code_of(sibling).and_then(local::desc) else { continue };
            let mut operands = function.blocks[b].insts[at].operands.clone();
            operands[dest_at] = Operand::Use(from);
            let rewritten = op_inst(sibling, desc, operands);
            function.blocks[b].insts[at] = rewritten;
            function.blocks[b].insts.remove(copy_at);
        }
    }
}

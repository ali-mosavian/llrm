//! Pointer arithmetic in an address register. The pin selects `add.l #4, %dN`
//! for `getelementptr i8, p, 4`, because the selector does not know a pointer
//! from an integer, and the allocator then moves the pointer through a data
//! register and back (`move.l %a0, %d2; add.l #4, %d2; move.l %d2, %a0`). Where
//! the value comes from an address register, goes only to copies, and the
//! condition codes the add sets are not read, it is `adda.l #4, %a0` (and
//! `addq.l #4, %a0` after the overlay).
//!
//! DEVIATE from the pin's output. Reason: smaller and faster, and the one move
//! that sets no flags is `movea`. Exit: never; a target choice, off with
//! `Options::overlay`.

use llrm_spike_mf::{Kind, Operand, Reg};

use crate::mfunc::{Class, MFunction};

/// Rewrites the adds and subtracts of constants that are pointer arithmetic.
pub fn address_arithmetic(function: &mut MFunction) {
    let count = function.classes.len();
    let mut defs: Vec<Option<(usize, usize)>> = vec![None; count];
    let mut copy_users = vec![0usize; count];
    let mut other_users = vec![0usize; count];
    for (at, block) in function.blocks.iter().enumerate() {
        for (i, inst) in block.insts.iter().enumerate() {
            for operand in &inst.operands {
                match operand {
                    Operand::Def(Reg::Virt(v)) => defs[*v as usize] = Some((at, i)),
                    Operand::Use(Reg::Virt(v)) if inst.kind == Kind::Copy => copy_users[*v as usize] += 1,
                    Operand::Use(Reg::Virt(v)) => other_users[*v as usize] += 1,
                    _ => {}
                }
            }
        }
        for phi in &block.phis {
            for (_, reg) in &phi.incoming {
                if let Reg::Virt(v) = reg {
                    other_users[*v as usize] += 1;
                }
            }
        }
    }
    // whether `v` is, through copies, a register the memory operands put in an
    // address register
    let from_address = |function: &MFunction, mut v: u32| {
        for _ in 0..8 {
            if function.classes[v as usize] == Class::Ar32 {
                return true;
            }
            let Some((at, i)) = defs[v as usize] else { return false };
            let inst = &function.blocks[at].insts[i];
            match (inst.kind, &inst.operands[..]) {
                (Kind::Copy, [_, Operand::Use(Reg::Virt(from))]) => v = *from,
                _ => return false,
            }
        }
        false
    };
    for at in 0..function.blocks.len() {
        for i in 0..function.blocks[at].insts.len() {
            let inst = &function.blocks[at].insts[i];
            let wide = match inst.kind {
                Kind::Op("ADD32di") => "ADD32ai",
                Kind::Op("SUB32di") => "SUB32ai",
                _ => continue,
            };
            let [Operand::Def(Reg::Virt(dst)), Operand::Use(Reg::Virt(src)), Operand::Imm(_)] = inst.operands[..]
            else {
                continue;
            };
            let flags_read = function.blocks[at].insts[i + 1..]
                .iter()
                .find(|later| later.reads_flags || later.sets_flags)
                .is_some_and(|later| later.reads_flags);
            if flags_read
                || other_users[dst as usize] != 0
                || copy_users[dst as usize] == 0
                || !from_address(function, src)
                // the add is the only reader of `src`, so the data class it
                // gave
                // `src` goes with it
                || other_users[src as usize] != 1
                || copy_users[src as usize] != 0
            {
                continue;
            }
            function.classes[dst as usize] = Class::Ar32;
            function.classes[src as usize] = Class::Ar32;
            let inst = &mut function.blocks[at].insts[i];
            inst.kind = Kind::Op(wide);
            inst.sets_flags = false;
        }
    }
}

/// Where an instruction's `(d8,An,Xn)` operand starts: the displacement, then
/// the base and the index.
fn indexed_operand(inst: &llrm_spike_mf::Inst) -> Option<usize> {
    let Kind::Op(name) = inst.kind else { return None };
    let &(_, _, operands) = crate::dagisel::asm::asm(name)?;
    let slot = operands.iter().find(|one| one.method.starts_with("printARII") && one.method.ends_with("Mem"))?;
    Some(usize::from(slot.at))
}

/// An index read after the add that steps it is the add's result with the
/// step taken from the displacement: `d1 = d0; d0 += 1; tst.b (1,%a0,%d1)` is
/// `d0 += 1; tst.b (%a0,%d0)`. The old index is then dead at the add, which
/// takes its register, and the copy goes.
///
/// DEVIATE from the pin, which leaves the address on the old index. Reason: a
/// loop that tests `p[i + 1]` after counting `i` moved the counter every trip.
/// Exit: never; a target choice, off with `Options::overlay`.
pub fn reassociate_indexed_displacement(function: &mut MFunction) {
    let count = function.classes.len();
    let mut readers = vec![0usize; count];
    for block in &function.blocks {
        for inst in &block.insts {
            for operand in &inst.operands {
                if let Operand::Use(Reg::Virt(v)) = operand {
                    readers[*v as usize] += 1;
                }
            }
        }
        for phi in &block.phis {
            for (_, reg) in &phi.incoming {
                if let Reg::Virt(v) = reg {
                    readers[*v as usize] += 1;
                }
            }
        }
    }
    for block in &mut function.blocks {
        for i in 0..block.insts.len() {
            let Some(at) = indexed_operand(&block.insts[i]) else { continue };
            let [Operand::Imm(disp), Operand::Use(Reg::Virt(_)), Operand::Use(Reg::Virt(index))] =
                block.insts[i].operands[at..at + 3]
            else {
                continue;
            };
            // the step before it, the only other reader of the index
            let Some(step) = (0..i).rev().find(|&j| {
                block.insts[j].operands.iter().any(|o| matches!(o, Operand::Def(Reg::Virt(d)) if *d == index))
                    || block.insts[j].operands.iter().any(|o| matches!(o, Operand::Use(Reg::Virt(u)) if *u == index))
            }) else {
                continue;
            };
            let (sign, [Operand::Def(Reg::Virt(next)), Operand::Use(Reg::Virt(from)), Operand::Imm(amount)]) = (
                match block.insts[step].kind {
                    Kind::Op("ADD32di") => 1,
                    Kind::Op("SUB32di") => -1,
                    _ => continue,
                },
                &block.insts[step].operands[..],
            ) else {
                continue;
            };
            let (next, from, moved) = (*next, *from, disp - sign * amount);
            if from != index || readers[index as usize] != 2 || !(-128..=127).contains(&moved) {
                continue;
            }
            // `next` must not be written again before the access
            if block.insts[step + 1..i]
                .iter()
                .any(|between| between.operands.iter().any(|o| matches!(o, Operand::Def(Reg::Virt(d)) if *d == next)))
            {
                continue;
            }
            block.insts[i].operands[at] = Operand::Imm(moved);
            block.insts[i].operands[at + 2] = Operand::Use(Reg::Virt(next));
        }
    }
}

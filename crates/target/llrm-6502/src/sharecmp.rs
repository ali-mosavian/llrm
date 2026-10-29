//! A second select on a compare already made reuses its carry.
//!
//! `max = a > b ? b : 0; a -= max; min = a > b ? 0 : a; b -= min` compares
//! twice because the subtraction between the selects sets the carry again. A
//! select leaves the carry alone (loads do not touch it), so the second select
//! moves up next to the first and reads the first compare; the second `lda a;
//! cmp b` goes.
//!
//! DEVIATE: no counterpart in LLVM, whose x86 selects read EFLAGS once per
//! `cmp` and whose `X86FlagsCopyLowering`/`MachineCSE` find the repeat; this
//! target's flags are not allocated, so the repeat is found by shape here.
//! Exit: MachineCSE over the flags register.

use llrm_spike_mf::{Inst, Kind, MFunction, Operand, Reg};

use crate::alloc::Class;
use crate::reg;

/// `carry` selects (`bcc`, `bcs`) are the ones that leave the flag alone.
fn carry_select(inst: &Inst) -> bool {
    inst.kind == Kind::Op("CMOV") && matches!(inst.operands.get(3), Some(Operand::Imm(2 | 3)))
}

fn is_copy_from_a(inst: &Inst) -> bool {
    matches!(
        &inst.operands[..],
        [Operand::Def(Reg::Virt(_)), Operand::Use(Reg::Phys(a))] if inst.kind == Kind::Copy && *a == reg::A
    )
}

/// The `(lhs, rhs)` of a `lda lhs; cmp rhs` pair that ends at `at`.
fn compare_at(
    insts: &[Inst],
    at: usize,
) -> Option<(Operand, Operand)> {
    let compare = insts.get(at).filter(|i| i.kind == Kind::Op("CMP"))?;
    let copy = insts.get(at.checked_sub(1)?).filter(|i| i.kind == Kind::Copy)?;
    match (&copy.operands[..], &compare.operands[..]) {
        ([Operand::Def(Reg::Phys(a)), lhs @ Operand::Use(_)], [_, _, rhs @ Operand::Use(Reg::Virt(_))])
            if *a == reg::A =>
        {
            Some((lhs.clone(), rhs.clone()))
        }
        _ => None,
    }
}

fn defines(
    inst: &Inst,
    operand: &Operand,
) -> bool {
    let Operand::Use(register) = operand else { return false };
    inst.operands.iter().any(|o| matches!(o, Operand::Def(d) if d == register))
}

/// `Some(true)` if `inst` sets `a` without reading it, `Some(false)` if it
/// reads it.
fn touches_a(inst: &Inst) -> Option<bool> {
    let is_a = |o: &Operand| {
        matches!(
            o,
            Operand::Use(Reg::Phys(r)) | Operand::Def(Reg::Phys(r)) if *r == reg::A
        )
    };
    let reads = inst.operands.iter().any(|o| matches!(o, Operand::Use(_)) && is_a(o));
    let sets = inst.operands.iter().any(|o| matches!(o, Operand::Def(_)) && is_a(o));
    (reads || sets).then_some(!reads)
}

/// Rewrites the blocks of `function`; whether anything changed.
pub fn share_compares(function: &mut MFunction<Class>) -> bool {
    let mut changed = false;
    for block in &mut function.blocks {
        'again: loop {
            let insts = &block.insts;
            for first in 1..insts.len() {
                let Some(keys) = compare_at(insts, first) else { continue };
                // the first compare, its select, and the copy of the select's
                // result
                let (Some(select), Some(saved)) = (insts.get(first + 1), insts.get(first + 2)) else { continue };
                if !carry_select(select) || !is_copy_from_a(saved) {
                    continue;
                }
                for second in first + 3..insts.len() {
                    if insts[second].call || insts[second].terminator {
                        break;
                    }
                    if compare_at(insts, second) != Some(keys.clone()) {
                        continue;
                    }
                    let (Some(again), Some(result)) = (insts.get(second + 1), insts.get(second + 2)) else { break };
                    if !carry_select(again) || !is_copy_from_a(result) {
                        break;
                    }
                    if second < first + 4 {
                        break;
                    }
                    let between = &insts[first + 3..second - 1];
                    let value_uses: Vec<&Operand> =
                        again.operands.iter().filter(|o| matches!(o, Operand::Use(Reg::Virt(_)))).collect();
                    let result_def = &result.operands[0];
                    let moved = |i: &Inst| {
                        i.call
                            || i.terminator
                            || i.side_effects
                            || [&keys.0, &keys.1].into_iter().chain(value_uses.iter().copied()).any(|o| defines(i, o))
                            || i.operands.iter().any(|o| match (o, result_def) {
                                (Operand::Use(a) | Operand::Def(a), Operand::Def(b)) => a == b,
                                _ => false,
                            })
                    };
                    // `a` is set before anything in between reads it, and
                    // nothing later reads the
                    // second compare's flags
                    let a_set_first = between.iter().find_map(touches_a).unwrap_or(true);
                    let flags_later = insts[second + 3..]
                        .iter()
                        .take_while(|i| {
                            !i.operands.iter().any(|o| matches!(o, Operand::Def(Reg::Phys(f)) if *f == reg::FLAGS))
                        })
                        .any(|i| {
                            i.operands.iter().any(|o| matches!(o, Operand::Use(Reg::Phys(f)) if *f == reg::FLAGS))
                        });
                    if between.iter().any(moved) || !a_set_first || flags_later {
                        break;
                    }
                    let mut next: Vec<Inst> = insts[..first + 3].to_vec();
                    next.push(again.clone());
                    next.push(result.clone());
                    next.extend_from_slice(between);
                    next.extend_from_slice(&insts[second + 3..]);
                    block.insts = next;
                    changed = true;
                    continue 'again;
                }
            }
            break;
        }
    }
    changed
}

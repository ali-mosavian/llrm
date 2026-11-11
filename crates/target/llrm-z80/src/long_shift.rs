//! A long doubled in a loop: one instruction, and sunk to the edge that reads it.
//!
//! # How it works
//! `x << 1` on a long is `sla lo; rl hi; adc hl, hl` in the flags: selection
//! gives `SHL16F` (the low word, carry out) and `ADC16Fhl` (the high word),
//! two instructions the carry in `f` binds. [`fuse`] makes them one `SHL32`
//! with no flag between its parts.
//!
//! Mid-end IR computes `x << 1` in the loop header ahead of the exit test, and
//! the exit returns the old `x`: both live across the test, four pairs on a
//! machine with three. The pin moves such a computation into the successor
//! that uses it (`MachineSinking::FindSuccToSinkTo`, MachineSink.cpp, before
//! phi elimination); an instruction that sets a flag the next one reads cannot
//! go alone, the fused one can. [`sink`] does that for the one case the
//! target meets: a single-predecessor successor (the back edge's block).
//!
//! DEVIATE: no shared MachineSink. Reason: the z80 flow runs on its own
//! `MFunction`, which has no sinking pass but the copies'. Exit: the z80 flow
//! on `flow::machine_with`.

use crate::mfunc::{Inst, Kind, MFunction, Operand, Reg};

/// `SHL16F`, the copies that bring in the high word, `ADC16Fhl`, as one
/// `SHL32 [lo', hl', f, lo, hl]` (the copy in and out of hl stay).
pub fn fuse(function: &mut MFunction) {
    for block in &mut function.blocks {
        let mut at = 0;
        while at < block.insts.len() {
            if block.insts[at].kind != Kind::Op("SHL16F") {
                at += 1;
                continue;
            }
            let [Operand::Def(low), Operand::Def(_), Operand::Use(from)] = block.insts[at].operands[..] else {
                at += 1;
                continue;
            };
            // only copies between, none of them writes the flags
            let Some(adc) = (at + 1..block.insts.len()).find(|&k| block.insts[k].kind != Kind::Copy) else {
                break;
            };
            if block.insts[adc].kind != Kind::Op("ADC16Fhl") {
                at += 1;
                continue;
            }
            let [Operand::Def(hl), Operand::Def(flags), Operand::Use(_), Operand::Use(_)] = block.insts[adc].operands[..]
            else {
                at += 1;
                continue;
            };
            let mut fused = Inst::new(
                Kind::Op("SHL32"),
                vec![
                    Operand::Def(low),
                    Operand::Def(hl),
                    Operand::Def(flags),
                    Operand::Use(from),
                    Operand::Use(hl),
                ],
            );
            fused.ties = vec![(0, 3), (1, 4)];
            fused.sets_flags = true;
            block.insts[adc] = fused;
            block.insts.remove(at);
            at = adc;
        }
    }
}

fn successors(
    function: &MFunction,
    at: usize,
) -> Vec<u32> {
    let block = &function.blocks[at];
    let mut out: Vec<u32> = block
        .insts
        .iter()
        .filter(|inst| inst.terminator)
        .flat_map(|inst| inst.operands.iter())
        .filter_map(|operand| if let Operand::Block(id) = operand { Some(*id) } else { None })
        .collect();
    if !block.insts.last().is_some_and(|inst| inst.barrier) && at + 1 < function.blocks.len() {
        out.push(function.blocks[at + 1].id);
    }
    out
}

fn reads(
    inst: &Inst,
    v: u32,
) -> bool {
    inst.operands.iter().any(|operand| matches!(operand, Operand::Use(Reg::Virt(u)) if *u == v))
}

/// Moves each `SHL32` of a block that branches, with the copies that carry its
/// results, into the successor whose only predecessor is the block, where
/// everything that reads them is: the exit path no longer computes it.
pub fn sink(function: &mut MFunction) {
    for b in 0..function.blocks.len() {
        let successors_of_b = successors(function, b);
        if successors_of_b.len() < 2 {
            continue;
        }
        let Some(shift) = function.blocks[b].insts.iter().position(|inst| inst.kind == Kind::Op("SHL32")) else {
            continue;
        };
        // the target: the one successor that only this block enters
        let predecessors = |id: u32| {
            (0..function.blocks.len()).filter(|&p| successors(function, p).contains(&id)).collect::<Vec<_>>()
        };
        let candidates: Vec<usize> = successors_of_b
            .iter()
            .copied()
            .filter(|&id| predecessors(id) == [b])
            .filter_map(|id| function.blocks.iter().position(|block| block.id == id))
            .collect();
        for target in candidates {
            if sink_into(function, b, target, shift) {
                break;
            }
        }
    }
}

/// One attempt: the unit of the `SHL32` at `shift` of block `b` into block `target`.
fn sink_into(
    function: &mut MFunction,
    b: usize,
    target: usize,
    shift: usize,
) -> bool {
    {
        if target == b || !function.blocks[target].phis.is_empty() {
            return false;
        }
        let insts = &function.blocks[b].insts;
        // the copy into hl just ahead of the shift, and the copy out of hl just after
        let mut unit = vec![shift];
        let Operand::Use(hl) = insts[shift].operands[4] else { continue };
        let Some(copy_in) = (0..shift).rev().find(|&k| {
            matches!(&insts[k].operands[..], [Operand::Def(d), Operand::Use(_)] if *d == hl && insts[k].kind == Kind::Copy)
        }) else {
            return false;
        };
        if insts[copy_in + 1..shift].iter().any(|inst| inst.operands.iter().any(|o| matches!(o, Operand::Def(r) | Operand::Use(r) if *r == hl)))
        {
            return false;
        }
        unit.push(copy_in);
        let Some(copy_out) = (shift + 1..insts.len()).find(|&k| {
            matches!(&insts[k].operands[..], [Operand::Def(_), Operand::Use(s)] if *s == hl && insts[k].kind == Kind::Copy)
        }) else {
            return false;
        };
        unit.push(copy_out);
        // the results' copies, followed while they stay in the block
        let mut defined: Vec<u32> = Vec::new();
        for &k in &unit {
            for operand in &insts[k].operands {
                if let Operand::Def(Reg::Virt(v)) = operand {
                    defined.push(*v);
                }
            }
        }
        let target_id = function.blocks[target].id;
        let used_in_the_target = |v: u32| {
            // every reader of `v` outside `members` must be in the target or a phi fed from it
            function.blocks.iter().enumerate().all(|(k, block)| {
                let phis_ok = block
                    .phis
                    .iter()
                    .all(|phi| phi.incoming.iter().all(|(pred, r)| *r != Reg::Virt(v) || function.blocks[*pred].id == target_id));
                let insts_ok = k == target || k == b || !block.insts.iter().any(|inst| reads(inst, v));
                phis_ok && insts_ok
            })
        };
        let mut grew = true;
        while grew {
            grew = false;
            for k in 0..insts.len() {
                if unit.contains(&k) || k < shift {
                    continue;
                }
                if let [Operand::Def(Reg::Virt(d)), Operand::Use(Reg::Virt(s))] = insts[k].operands[..]
                    && insts[k].kind == Kind::Copy
                    && defined.contains(&s)
                    && !defined.contains(&d)
                {
                    defined.push(d);
                    unit.push(k);
                    grew = true;
                }
            }
        }
        // what the unit leaves for others: every virtual result is read only past the block's branch
        let in_unit = |k: usize| unit.contains(&k);
        let leaks = defined.iter().any(|&v| {
            let inside = insts.iter().enumerate().any(|(k, inst)| !in_unit(k) && reads(inst, v));
            inside || !used_in_the_target(v)
        });
        if leaks {
            return false;
        }
        // the registers the unit writes and reads must not be touched, before a write, by what it passes
        let first = *unit.iter().min().unwrap();
        let phys: Vec<Reg> = unit
            .iter()
            .flat_map(|&k| insts[k].operands.iter())
            .filter_map(|operand| match operand {
                Operand::Def(r @ Reg::Phys(_)) | Operand::Use(r @ Reg::Phys(_)) => Some(*r),
                _ => None,
            })
            .collect();
        let mut blocked = false;
        for &r in &phys {
            for (k, inst) in insts.iter().enumerate().skip(first) {
                if in_unit(k) {
                    continue;
                }
                let touches = inst.operands.iter().find_map(|o| match o {
                    Operand::Def(x) if *x == r => Some(true),
                    Operand::Use(x) if *x == r => Some(false),
                    _ => None,
                });
                match touches {
                    Some(true) => break,
                    Some(false) => blocked = true,
                    None => {}
                }
                if inst.call {
                    blocked = true;
                }
            }
        }
        // a stray use of a virtual register the unit reads is fine (it is still defined); a def of one is not
        let reads_virtual: Vec<u32> = unit
            .iter()
            .flat_map(|&k| insts[k].operands.iter())
            .filter_map(|o| if let Operand::Use(Reg::Virt(v)) = o { Some(*v) } else { None })
            .collect();
        let overwritten = insts.iter().enumerate().any(|(k, inst)| {
            !in_unit(k)
                && inst.operands.iter().any(|o| matches!(o, Operand::Def(Reg::Virt(v)) if reads_virtual.contains(v)))
        });
        if blocked || overwritten {
            return false;
        }
        unit.sort_unstable();
        let moved: Vec<Inst> = unit.iter().map(|&k| function.blocks[b].insts[k].clone()).collect();
        for &k in unit.iter().rev() {
            function.blocks[b].insts.remove(k);
        }
        let to = &mut function.blocks[target].insts;
        for (n, inst) in moved.into_iter().enumerate() {
            to.insert(n, inst);
        }
        return true;
    }
}

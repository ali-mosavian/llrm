//! A long doubled: one instruction on whichever two pairs hold its words, and
//! in a loop sunk to the edge that reads it.
//!
//! # How it works
//! `x << 1` on a long is `sla lo; rl hi` in the flags: selection gives
//! `ADD16F` (the low word, carry out) and `ADC16F` (the high word), two
//! instructions that need `hl` for each word in turn and the carry in `f`
//! between them. [`fuse`] makes them one `SHL32` over the two words' own
//! pairs with no flag between its parts: `add hl, hl` when a word is in `hl`,
//! `sla`/`rl` of its bytes otherwise, so `x << 1` with `x` in `hl:de` is
//! `add hl, hl; rl e; rl d` and the words never trade places through a third
//! pair.
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

/// The vreg a chain of copies starts from.
fn root(
    function: &MFunction,
    mut v: u32,
) -> u32 {
    'walk: loop {
        for block in &function.blocks {
            for inst in &block.insts {
                if let (Kind::Copy, [Operand::Def(Reg::Virt(d)), Operand::Use(Reg::Virt(u))]) =
                    (inst.kind, &inst.operands[..])
                    && *d == v
                {
                    v = *u;
                    continue 'walk;
                }
            }
        }
        return v;
    }
}

/// The virtual register copied into the physical `reg` by the nearest copy
/// before `at`.
fn copied_into(
    insts: &[Inst],
    at: usize,
    reg: Reg,
) -> Option<(usize, u32)> {
    let k = (0..at).rev().find(|&k| insts[k].operands.iter().any(|o| matches!(o, Operand::Def(r) if *r == reg)))?;
    match (insts[k].kind, &insts[k].operands[..]) {
        (Kind::Copy, [Operand::Def(_), Operand::Use(Reg::Virt(v))]) => Some((k, *v)),
        _ => None,
    }
}

/// `ADD16F x, x` and the `ADC16F y, y` after it (a long added to itself) as one
/// `SHL32 [lo', hi', f, x, y]`: the copies in and out of hl around each go
/// with the instructions, the results are the fused instruction's own.
fn fuse_block(
    function: &mut MFunction,
    b: usize,
) -> bool {
    let insts = &function.blocks[b].insts;
    for add in 0..insts.len() {
        let [Operand::Def(hl), Operand::Def(_), Operand::Use(_), Operand::Use(Reg::Virt(x))] = insts[add].operands[..]
        else {
            continue;
        };
        if insts[add].kind != Kind::Op("ADD16F") {
            continue;
        }
        let Some((into, held)) = copied_into(insts, add, hl) else { continue };
        if root(function, held) != root(function, x) {
            continue;
        }
        // the low word's copy out of hl, then the high word's copy in and its
        // ADC16F
        let Some(out) = (add + 1..insts.len()).find(|&k| {
            matches!(
                &insts[k].operands[..],
                [Operand::Def(Reg::Virt(_)), Operand::Use(r)] if *r == hl && insts[k].kind == Kind::Copy
            )
        }) else {
            continue;
        };
        let Operand::Def(low @ Reg::Virt(low_id)) = insts[out].operands[0] else { continue };
        let Some(adc) = (out + 1..insts.len()).find(|&k| insts[k].kind != Kind::Copy) else { continue };
        let [Operand::Def(_), Operand::Def(flags), Operand::Use(_), Operand::Use(Reg::Virt(y)), Operand::Use(_)] =
            insts[adc].operands[..]
        else {
            continue;
        };
        if insts[adc].kind != Kind::Op("ADC16F") {
            continue;
        }
        let Some((copy_in, held_high)) = copied_into(insts, adc, hl) else { continue };
        if root(function, held_high) != root(function, y)
            || insts[out + 1..adc].iter().any(|i| i.kind != Kind::Copy || reads(i, low_id))
        {
            continue;
        }
        // the high word leaves hl by the next copy
        let Some(copy_out) = insts.get(adc + 1).filter(|i| {
            i.kind == Kind::Copy
                && matches!(
                    &i.operands[..],
                    [Operand::Def(Reg::Virt(_)), Operand::Use(r)] if *r == hl
                )
        }) else {
            continue;
        };
        let Operand::Def(high) = copy_out.operands[0] else { continue };
        // the high word's `rl` leaves the other flags of its byte, not of the
        // word: no one may read them
        if insts[adc + 2..]
            .iter()
            .take_while(|i| !i.operands.iter().any(|o| matches!(o, Operand::Def(r) if *r == flags)))
            .any(|i| i.reads_flags())
        {
            continue;
        }
        let mut fused = Inst::new(
            Kind::Op("SHL32"),
            vec![
                Operand::Def(low),
                Operand::Def(high),
                Operand::Def(flags),
                Operand::Use(Reg::Virt(x)),
                Operand::Use(Reg::Virt(y)),
            ],
        );
        fused.ties = vec![(0, 3), (1, 4)];
        fused.sets_flags = true;
        let insts = &mut function.blocks[b].insts;
        insts[adc] = fused;
        for k in [adc + 1, copy_in, out, add, into] {
            insts.remove(k);
        }
        return true;
    }
    false
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

/// Every long doubled is fused; one in a block that branches is also moved
/// with the copies that carry its results into the successor whose only
/// predecessor is the block, where everything that reads them is, so the exit
/// path no longer computes it.
pub fn double_and_sink(function: &mut MFunction) {
    if std::env::var("LLRM_NO_Z80_SHL32").is_ok() {
        return;
    }
    for b in 0..function.blocks.len() {
        while fuse_block(function, b) {
            if successors(function, b).len() >= 2 {
                sink_block(function, b);
            }
        }
    }
}

fn sink_block(
    function: &mut MFunction,
    b: usize,
) -> bool {
    let successors_of_b = successors(function, b);
    let Some(shift) = function.blocks[b].insts.iter().position(|inst| inst.kind == Kind::Op("SHL32")) else {
        return false;
    };
    // the target: a successor that only this block enters
    let predecessors =
        |id: u32| (0..function.blocks.len()).filter(|&p| successors(function, p).contains(&id)).collect::<Vec<_>>();
    let candidates: Vec<usize> = successors_of_b
        .iter()
        .copied()
        .filter(|&id| predecessors(id) == [b])
        .filter_map(|id| function.blocks.iter().position(|block| block.id == id))
        .collect();
    candidates.into_iter().any(|target| sink_into(function, b, target, shift))
}

/// One attempt: the unit of the `SHL32` at `shift` of block `b` into block
/// `target`.
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
        let mut unit = vec![shift];
        // the results' copies, followed while they stay in the block
        let mut defined: Vec<u32> = Vec::new();
        for operand in &insts[shift].operands {
            if let Operand::Def(Reg::Virt(v)) = operand {
                defined.push(*v);
            }
        }
        let target_id = function.blocks[target].id;
        let used_in_the_target = |v: u32| {
            // every reader of `v` outside `members` must be in the target or a
            // phi fed from it
            function.blocks.iter().enumerate().all(|(k, block)| {
                let phis_ok = block.phis.iter().all(|phi| {
                    phi.incoming.iter().all(|(pred, r)| *r != Reg::Virt(v) || function.blocks[*pred].id == target_id)
                });
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
        // what the unit leaves for others: every virtual result is read only
        // past the block's branch
        let in_unit = |k: usize| unit.contains(&k);
        let leaks = defined
            .iter()
            .any(
                |&v| {
                    let inside = insts.iter().enumerate().any(|(k, inst)| !in_unit(k) && reads(inst, v));
                    inside || !used_in_the_target(v)
                },
            );
        if leaks {
            return false;
        }
        // the registers the unit writes and reads must not be touched, before a
        // write, by what it passes
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
                let touches = inst
                    .operands
                    .iter()
                    .find_map(
                        |o| match o {
                            Operand::Def(x) if *x == r => Some(true),
                            Operand::Use(x) if *x == r => Some(false),
                            _ => None,
                        },
                    );
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
        // a stray use of a virtual register the unit reads is fine (it is still
        // defined); a def of one is not
        let reads_virtual: Vec<u32> = unit
            .iter()
            .flat_map(|&k| insts[k].operands.iter())
            .filter_map(|o| if let Operand::Use(Reg::Virt(v)) = o { Some(*v) } else { None })
            .collect();
        let overwritten = insts.iter().enumerate().any(|(k, inst)| {
            k > first
                && !in_unit(k)
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

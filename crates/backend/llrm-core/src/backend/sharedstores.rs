//! Tuned for size, a run of stores of one literal is the literal in a dead
//! register, loaded once, then register stores: `mov dword ptr [bp-8],0` is 8
//! bytes, `mov [bp-8],eax` 4, and the one `xor eax,eax` 3. Two stores or more
//! (one dword zero, too) are fewer bytes; the load costs a clock, so -Os only.

use std::sync::Arc;

use llrm_lir::registers::RegId;

use crate::backend::classes::RegisterClasses;
use crate::backend::cpu::Profile;
use crate::backend::lanes::Lanes;
use crate::backend::peephole::{_flags_dead_after, _flags_live_out, _lanes, _register_effects, DeadAfter, id};
use crate::backend::{liveness, masm, regthrash, select, target};
use crate::model::ir::{Imm, Loc, Operation, Reg, Semantics};
use crate::model::lir::{Insn, LirBody};

/// The cell and literal of a plain store of an immediate that no relocation
/// moves.
fn literal(one: &Insn) -> Option<(u32, i64)> {
    let what = one.what.as_ref()?;
    if what.op != Operation::Move
        || what.name.as_deref() != Some("mov")
        || !one.clobbers.is_empty()
        || one.symbol == Some(true)
    {
        return None;
    }
    let ([Loc::Mem(cell)], [Loc::Imm(Imm { value, width, address: None })]) =
        (what.dests.as_slice(), what.sources.as_slice())
    else {
        return None;
    };
    (cell.width == *width && [1, 2, 4].contains(width)).then_some((*width, *value))
}

fn bytes(
    bits: u32,
    what: &Semantics,
) -> Option<usize> {
    select::priced_in(bits, what, 0, None, false, false, None).map(|code| code.code.len())
}

/// `body` with each run of equal literal stores that a dead register makes
/// shorter shared.
pub fn shared(
    body: &LirBody,
    cpu: &Profile,
    saved: &[RegId],
    classes: &RegisterClasses,
) -> LirBody {
    if !cpu.size {
        return body.clone();
    }
    // What the epilogue saves is what its convention preserves of the registers
    // the body names (`masm::_frame_parts`): one it does not name would be
    // pushed and popped for the run.
    let used = masm::_roots(body);
    let scratch: Vec<RegId> =
        classes.available.iter().copied().filter(|one| !saved.contains(one) || used.contains(one)).collect();
    let exits = liveness::dead_at_exit(body);
    let flags_out = _flags_live_out(body);
    let blocks = body
        .blocks
        .iter()
        .map(|block| {
            let dead = regthrash::_dead_after(body.bits, block, exits[&block.at].clone());
            let flags_dead_out = flags_out.get(&block.at).is_some_and(|lanes| lanes.is_empty());
            let flags_dead = _flags_dead_after(body.bits, block, flags_dead_out);
            let mut insns: Vec<Arc<Insn>> = Vec::with_capacity(block.insns.len());
            let mut at = 0;
            while at < block.insns.len() {
                let first = &block.insns[at];
                let Some((width, value)) = literal(first) else {
                    insns.push(Arc::clone(first));
                    at += 1;
                    continue;
                };
                // Zero is any width's; another literal is shared at its own.
                let run = block.insns[at..]
                    .iter()
                    .take_while(|one| {
                        literal(one).is_some_and(|(wide, same)| same == value && (value == 0 || wide == width))
                    })
                    .count();
                match shared_run(body.bits, &block.insns[at..at + run], value, &scratch, &dead, &flags_dead) {
                    Some(made) => insns.extend(made),
                    None => insns.extend(block.insns[at..at + run].iter().cloned()),
                }
                at += run;
            }
            block.with_insns(insns)
        })
        .collect();
    body.with_blocks(blocks)
}

fn shared_run(
    bits: u32,
    run: &[Arc<Insn>],
    value: i64,
    scratch: &[RegId],
    dead: &DeadAfter,
    flags_dead: &crate::support::hash::HashSet<usize>,
) -> Option<Vec<Arc<Insn>>> {
    let last = run.last()?;
    let widths: Vec<u32> = run.iter().map(|one| literal(one).expect("a literal store").0).collect();
    let width = *widths.iter().max()?;
    let reads = run
        .iter()
        .filter_map(|one| _register_effects(bits, one, false, true))
        .fold(Lanes::new(), |all, (reads, _)| all.or(&reads));
    let named = |width: u32, full: RegId| -> Option<Reg> {
        let register = target::named(full, i64::from(width));
        (target::width_of(register) == Some(i64::from(width))).then_some(Reg { register, width })
    };
    let (full, register) = scratch
        .iter()
        .find_map(
            |full| {
                let register = named(width, *full)?;
                (_lanes(register.register).is_subset(&dead[&id(last)]) && _lanes(register.register).is_disjoint(&reads))
                    .then_some((*full, register))
            },
        )?;
    let destination = Loc::Reg(register);
    let zero = value == 0 && flags_dead.contains(&id(&run[0]));
    let load = Semantics {
        name: Some(if zero { "xor" } else { "mov" }.to_owned()),
        dests: vec![destination.clone()],
        sources: if zero {
            vec![destination.clone(), destination.clone()]
        } else {
            vec![Loc::Imm(Imm { value, width, address: None })]
        },
        ..Semantics::new(if zero { Operation::Binary } else { Operation::Move })
    };
    let stores: Vec<Semantics> = run
        .iter()
        .zip(&widths)
        .map(|(one, width)| {
            Some(Semantics {
                sources: vec![Loc::Reg(named(*width, full)?)],
                ..one.what.clone().expect("a literal store")
            })
        })
        .collect::<Option<_>>()?;
    let before: usize =
        run.iter().map(|one| bytes(bits, one.what.as_ref().expect("a literal store"))).sum::<Option<usize>>()?;
    let after = bytes(bits, &load)? + stores.iter().map(|what| bytes(bits, what)).sum::<Option<usize>>()?;
    if after >= before {
        return None;
    }
    let first = &run[0];
    let made = Arc::new(Insn {
        line: first.line,
        ..Insn::new(first.at, Some((first.at, first.at)), Some(load), vec![], vec![])
    });
    let mut out = vec![made];
    out.extend(run.iter().zip(stores).map(|(one, what)| Arc::new(Insn { what: Some(what), ..(**one).clone() })));
    Some(out)
}

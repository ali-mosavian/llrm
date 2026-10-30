//! At -Os, a run of stores of one literal becomes the literal loaded once into
//! a dead register, then register stores.
//!
//! # How it works
//! `mov dword ptr [bp-8],0` is 8 bytes, `mov [bp-8],eax` is 4 and the one `xor
//! eax,eax` is 3, so two stores or more win.
//!
//! ```text
//!  before                          after
//!  mov dword ptr [bp-8],0          xor eax,eax            ; 3 bytes
//!  mov dword ptr [bp-4],0          mov [bp-8],eax         ; 4
//!  mov dword ptr [bp-12],0         mov [bp-4],eax         ; 4
//!                                  mov [bp-12],eax        ; 4
//!  24 bytes                        15 bytes
//! ```
//!
//! A run is consecutive plain `mov` stores of an immediate in one block, with
//! no relocation on the literal. All stores of a run hold the same value; zero
//! may mix widths, any other literal needs the same width. The register is
//! taken from the allocatable ones, skipping a register the epilogue would have
//! to save and the body does not already name (it would be pushed and popped
//! for the run). It must be dead after the last store and not read by any store
//! of the run (an address register). It is as wide as the widest store;
//! narrower stores use the matching sub-register.
//!
//! The load is `xor r,r` when the literal is zero and the flags are dead at the
//! run's start, else `mov r,imm`. The change is made only if the encoded sizes,
//! priced by the selector, are strictly smaller than the run's.
//!
//! # Example
//! Four stores of zero through `BX`, with `AX` dead after the argument is
//! copied. Before, the input of `Peephole`:
//!
//! ```lir
//! === ParallelCopy
//!   _f@3: 7 instructions
//!     block 0x0000
//!       0x0000     ; inserted; delivers v2@AX
//!       0x0000  BX := mov AX  ; inserted
//!       0x0001  [None v1@BX] := mov 0x0  ; inserted
//!       0x0003  [None v1@BX] := mov 0x0  ; inserted
//!       0x0005  [None v1@BX] := mov 0x0  ; inserted
//!       0x0007  [None v1@BX] := mov 0x0  ; inserted
//! ```
//!
//! After:
//!
//! ```lir
//! === Peephole
//!   _f@3: 8 instructions
//!     block 0x0000
//!       0x0000  BX := mov AX  ; inserted
//!       0x0001  AX := xor AX, AX  ; inserted
//!       0x0001  [None v1@BX] := mov AX  ; inserted
//!       0x0003  [None v1@BX] := mov AX  ; inserted
//!       0x0005  [None v1@BX] := mov AX  ; inserted
//!       0x0007  [None v1@BX] := mov AX  ; inserted
//! ```
//!
//! One `xor` and four register stores replace four immediate stores. The dump
//! omits the stores' displacements.
//!
//! From `ISEL_DUMP=1` of `void f(struct s *p)` storing 0 to four `int` fields
//! at -Os -m16.
//!
//! # Runs
//! A sub-pass of [`crate::backend::peephole`], once per body, after
//! `zeroed_loads` and before `machinedce`. It returns the body unchanged unless
//! the CPU profile is sized for bytes (`cpu.size`, -Os): the load costs a
//! clock.

use std::sync::Arc;

use llrm_lir::registers::RegId;
use llrm_lir::registers::Regs;

use crate::backend::classes::RegisterClasses;
use crate::backend::cpu::Profile;
use crate::backend::lanes::Lanes;
use crate::backend::peephole::{_flags_dead_after, _flags_live_out, _lanes, _register_effects, DeadAfter, id};
use crate::backend::{liveness, masm, regthrash, select};
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
    let regs = body.regs();
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
            let dead = regthrash::_dead_after(regs, body.bits, block, exits[&block.at].clone());
            let flags_dead_out = flags_out.entering.get(&block.at).is_some_and(|lanes| lanes.is_empty());
            let flags_dead = _flags_dead_after(regs, body.bits, block, flags_dead_out);
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
                match shared_run(regs, body.bits, &block.insns[at..at + run], value, &scratch, &dead, &flags_dead) {
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
    regs: Regs,
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
        .filter_map(|one| _register_effects(regs, bits, one, false, true))
        .fold(Lanes::new(), |all, (reads, _)| all.or(&reads));
    let named = |width: u32, full: RegId| -> Option<Reg> {
        let register = regs.named(full, i64::from(width));
        (regs.width_of(register) == Some(i64::from(width))).then_some(Reg { register, width })
    };
    let (full, register) = scratch
        .iter()
        .find_map(
            |full| {
                let register = named(width, *full)?;
                (_lanes(regs, register.register).is_subset(&dead[&id(last)])
                    && _lanes(regs, register.register).is_disjoint(&reads))
                .then_some((*full, register))
            },
        )?;
    let destination = Loc::Reg(register);
    let zero = value == 0 && flags_dead.contains(&id(&run[0]));
    let load = Semantics {
        name: Some(if zero { "xor" } else { "mov" }.into()),
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
        line: run.iter().skip(1).fold(first.line, |line, one| crate::model::lir::merged_line(line, one.line)),
        ..Insn::new(first.at, Some((first.at, first.at)), Some(load), vec![], vec![])
    });
    let mut out = vec![made];
    out.extend(run.iter().zip(stores).map(|(one, what)| Arc::new(Insn { what: Some(what), ..(**one).clone() })));
    Some(out)
}

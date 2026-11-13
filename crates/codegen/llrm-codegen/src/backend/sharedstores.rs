//! At -Os (and at -O1 on a target that allows it), a run of stores of one
//! literal becomes the literal loaded once into a dead register, then register
//! stores.
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
    // A qword zero is the longest of them (`mov qword ptr [rsp+8],0` is 9
    // bytes, the register store 5) and shares the register of a narrower
    // zero, as every zero does; any other qword literal would need a 64-bit
    // load.
    (cell.width == *width && ([1, 2, 4].contains(width) || (*width == 8 && *value == 0))).then_some((*width, *value))
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
    // Where the target allows it (`share_literal_stores`) a run is also shared
    // at -O1, where bytes decide (`Profile::bytes_strict`).
    if !cpu.size && !(cpu.share_literal_stores && (cpu.speed_rate || cpu.bytes_strict())) {
        return body.clone();
    }
    // DEVIATE ISS1 (owner x86-m32, exit 2026-11-30): at -O2/-O3 the load costs
    // a clock a run, so a run is shared only outside any cycle (it runs
    // once a call) and where its bytes at the speed rate outweigh that
    // clock. DEVIATE ISS2 (owner x87, exit 2026-11-20, ruled 2026-11-09; the
    // lone store dropped 2026-11-11, oversight): at -O1 the same rule with
    // no rate, so a run of two or more outside any cycle is shared where it
    // saves a byte (nbody -O1 kept 24 immediate stores of its f64
    // constants, +82 B against gcc's 427 B with 3.8% fewer clocks; the pin
    // keeps them too). Every changed cell is slower by the `xor` (executed
    // clocks, tests/run/c at m32 -O1, cell +clk -B): bitcount_u32 +1 -2,
    // bitcount_u64 +1 -6, clzll_ctzll_64 +3 -6, div_constants_long +14 -28,
    // div_constants_short +12 -24, div_constants_ulong +10 -20,
    // double_across_a_call_result +1 -6, fcmp_all +1 -3, frameless +1 -2,
    // memfuncs_runtime +1 -2, memory_expansion +69 -1944, register_upper_half
    // +1 -2, spill_store_order +5 -2, u64_shift_select_arms +1 -2,
    // wrapv_overflow_check +1 -6. No cell above +14 clk but memory_expansion.
    let speed = !cpu.size;
    let cyclic = if speed { blocks_in_cycles(body) } else { Default::default() };
    let rate = if speed && cpu.speed_rate { cpu.speed_milliclocks_per_byte() } else { 0 };
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
            if cyclic.contains(&block.at) {
                return block.clone();
            }
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
                // A lone store is the pin's `mov m, imm` where no rate pays for
                // the load (-O1: +1 clk for 3 B).
                let shared = (run > 1 || !speed || rate > 0)
                    .then(|| {
                        shared_run(
                            regs,
                            body.bits,
                            &block.insns[at..at + run],
                            value,
                            &scratch,
                            &dead,
                            &flags_dead,
                            rate,
                        )
                    })
                    .flatten();
                match shared {
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
    rate: i64,
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
    // A clock for the load against `rate` thousandths of a clock a byte saved
    // (0: any byte saved).
    if after >= before || ((before - after) as i64) * rate < rate.min(1) * 1000 {
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

/// The blocks on a cycle: a store there runs more than once a call, so a clock
/// for the load is not free. One walk over the strongly connected components
/// (`scc_iterator` is LLVM's), a block being on a cycle when its component has
/// more than one block or it branches to itself.
pub(crate) fn blocks_in_cycles(body: &LirBody) -> crate::support::hash::HashSet<i64> {
    let place: crate::support::hash::HashMap<i64, usize> =
        body.blocks.iter().enumerate().map(|(index, block)| (block.at, index)).collect();
    let successors: Vec<Vec<usize>> =
        body.blocks.iter().map(|block| block.succ.iter().filter_map(|at| place.get(at).copied()).collect()).collect();
    let (component, cyclic) = llrm_mir::callgraph::strong_components(&successors);
    body.blocks.iter().enumerate().filter(|(index, _)| cyclic[component[*index]]).map(|(_, block)| block.at).collect()
}

# Code generation improvements

Candidate changes to llrm's backend, drawn from LLVM (clang `-m16`), gcc-ia16
and Open Watcom's 16-bit code generator. Each is ranked by expected payoff on
the 486 and P5, which llrm targets.

Links are pinned: llrm at [`079fceb3`][ll], Open Watcom v2 at
[`703e1ae2`][ow], gcc-ia16 at [`9a553954`][gi], and LLVM at
[`llvmorg-20.1.8`][lv], the clang version measured below.

## Overview

| # | Change | Source | Payoff | Effort |
|---|---|---|---|---|
| 1 | [Keep x87 values on the stack across a loop](#1-x87-values-across-a-loop) | Open Watcom, LLVM | Large on floating loops (`nbody` 1.74×) | Medium |
| 2 | [Same-segment far calls as `push cs; call near`](#2-same-segment-far-calls) | Microsoft LINK, Open Watcom, gcc-ia16 | ≈12 clocks per call on a 486 | Low |
| 3 | [One index register shared by sibling induction variables](#3-shared-index-registers) | Open Watcom, LLVM | Removes spilled pointers (`nbody`) | Low to medium |
| 4 | [Scheduling around address generation stalls](#4-address-generation-stalls) | Open Watcom | 1 clock per stall on 486 and P5 | Medium |
| 5 | [x87 interleaving with lazy `fxch`](#5-x87-interleaving) | Open Watcom, LLVM | P5 floating throughput | High |
| 6 | [Register allocator pieces not yet ported](#6-register-allocator) | LLVM | Unmeasured | High |
| 7 | [Smaller items](#7-smaller-items) | All three | Small each | Low |
| 8 | [Not worth porting](#8-not-worth-porting) | gcc-ia16, Open Watcom | None on a 486 | — |

## Measurements

Seven kernels from [`bench/c`][bench] were built three ways, with llrm at
`1ded74a4`. `llrm-c --opt --cpu 486` produced medium-model far objects, linked with Microsoft LINK.
`clang -m16 -march=i486 -O2` and `ia16-elf-gcc -march=i286 -O2` produced
tiny-model `.COM` files. Each kernel was its own translation unit, so no
compiler could see the harness's constant arguments. Each run timed the
kernel with RDTSC under DOSBox-X on the pinned machine
([`conf/pinned.conf`][pinned], since removed from `main` with the Python
tooling). Its normal core charges about one cycle per
instruction, so the counts are executed instructions, not 486 clocks. Every
answer matched [`bench/c/expected.json`][expected].

| Kernel | llrm | clang | gcc-ia16 | llrm / clang |
|---|---|---|---|---|
| sieve | 12,212 | 18,428 | 24,482 | 0.66 |
| crc | 483 | 444 | 1,338 | 1.09 |
| mandel | 184,010 | 170,964 | 712,032 | 1.08 |
| floats | 7,037 | 7,038 | — | 1.00 |
| lru | 142 | 90 | 264 | 1.58 |
| nbody | 92,976 | 53,387 | — | 1.74 |

gcc-ia16 stops at the 286 and has no x87, so it has no floating results. Its
integer results reflect the missing 32-bit registers more than its optimizer.
`matmul` and `shellsort` were left out: `llrm-c` rejects both on this commit
with "value#… is read but never defined".

## 1. x87 values across a loop

**Evidence.** In `nbody`'s inner loop, llrm stores `dx`, `dy` and `scale` to
the frame and reloads them for every use: about 60 instructions per
iteration. clang keeps them in `st(n)`, and loads `x[i]` and `y[i]` once per
outer iteration: 25 instructions.

**llrm today.** Two layers cause this.

- *Semantics.* Floating locals stay in memory. [`promote::_cell`][ll-promote]
  promotes integer loads and stores only. Float store-to-load forwarding
  ([`_exact_stored_load`][ll-exact]) fires only when the 80-bit value is
  proven exact in 64 bits, because every `fstp qword` rounds and the model is
  strict extended precision ([floating-environment.md][ll-fenv],
  [exact-floating-store-reuse.md][ll-exactdoc]).
- *Allocation.* x87 allocation is per region.
  - A region is a chain of blocks that ends at a call, a barrier or a
    physical ST operand ([`floatregions::boundary`][ll-boundary]).
  - The stack must be empty at region edges; otherwise allocation refuses
    with "floating stack live-out requires cross-block allocation"
    ([`floatalloc.rs`][ll-refuse]).
  - A value crossing a region gets a 10-byte frame cell
    ([`floatregions::bridged`][ll-bridged]).
  - Inside a region, [`retain_home`][ll-retain] keeps at most one reloadable
    value on the stack, and only when its first use is a self-use such as
    `x*x`. Overflow spills Belady-style ([`room`][ll-room]).

**Open Watcom.** [`CacheTemps`][ow-cache] finds single-block loops and keeps
their floating memory variables on the stack for the whole loop: loaded once
in the preheader ([`InitGlobalTemps`][ow-init]) and stored once in the exit
([`FiniGlobalTemps`][ow-fini]). [`StackBetween`][ow-between] keeps a variable
on the stack between its first and last use in a block when the depth
allows. Both run only under `-or`. The base allocator,
[`FPRegAlloc`][ow-fpalloc], refuses a value used in another block
([`USE_IN_ANOTHER_BLOCK`][ow-another]) or live across a call
([`NoStackAcrossCalls`][ow-calls]), which is the same rule as llrm's.

**LLVM.** [`X86FloatingPoint.cpp`][lv-fp] assigns every edge bundle a fixed
stack order. Each block is stackified against its incoming bundle and shuffled
into its outgoing one, so values stay in `st(n)` across any CFG, loops
included.

**Plan.**

1. Decide the precision policy. Keeping `dx` in `st(n)` skips the rounding a
   `double` store performs; clang, Open Watcom and C's `FLT_EVAL_METHOD == 2`
   all accept this. Make it opt-in, or the default for the C frontend only.
   Under it, `promote` can treat `Fstore`/`Fload` of a local like integer
   cells.
2. Port `CacheTemps` first: a single-block loop with no call gets a fixed
   stack shape at its header and latch, loaded in the preheader and stored in
   the exit. This fits the region model with one new contract at the loop
   edge.
3. Relax `retain_home` to keep more than one value, and values whose first
   use is not a self-use, while the depth budget allows.
4. Later, LLVM's edge bundles for general cross-block stackification.

**Payoff.** About an `FLD` plus an `FSTP` saved per value per iteration,
roughly 11 clocks on a 486. It is most of `nbody`'s gap.

## 2. Same-segment far calls

**Evidence.** llrm always emits a far call, `9A`, for a far callee
([`omfwrite.rs`][ll-9a]). A BASIC procedure is far so other modules can call
it, but most calls come from its own module and code segment.

**Sources.**

- Microsoft LINK 5.31 (VBDOS) and QB 4.5's LINK both accept
  `/FARCALLTRANSLATION`. It rewrites a far call whose target lies in the
  caller's segment into `push cs; call near`. The option string is present in
  both binaries; its effect on llrm's objects is unverified.
- Open Watcom emits the same form itself for a same-segment callee
  ([`OC_DEST_CHEAP`][ow-cheap], [`_OutCCyp`][ow-outccyp]), and turns a call
  followed by a return into a near jump ([`RetAftrCall`][ow-retaftr]). It
  marks each code segment with the `LDIR_OPT_FAR_CALLS` comment
  ([`x86omf.c`][ow-comment]), and its linker then rewrites every far call and
  jump that lands in the same final segment ([`FarCallOpt`][ow-farcallopt]).
- gcc-ia16 uses `pushw %cs` plus a near call for a far function in the same
  section ([`ia16_get_call_expansion`][gi-call]).

**Plan.**

1. Link one QB program with `/FARCALLTRANSLATION` and read the linked code.
   If LINK rewrites llrm's calls, pass the switch by default.
2. Otherwise emit the form in `omfwrite` when the callee is in the caller's
   segment. A tail call becomes a near jump, as in `RetAftrCall`.

**Payoff.** A real-mode far call costs about 18 clocks on a 486; `push cs`
plus a near call costs about 6. The form is one byte longer. It complements
QuickrBASIC's `PRIVATE`, which makes internal procedures fully near.

## 3. Shared index registers

**Evidence.** In `nbody`'s `j` loop, strength reduction creates four pointer
induction variables, for `x[j]`, `y[j]`, `vx[j]` and `vy[j]`. Pointers into
frame arrays may only use SI or DI, so two spill to the frame and the array
bases are rebuilt with `lea` on every iteration. clang uses one index
register for all four, `[esp+8*esi+disp]`.

**llrm today.** [`strength::reduced`][ll-reduced] chooses candidates
([`_candidates`][ll-candidates]) against a budget: the lesser of the
register capacity less 2 ([`_RESERVE`][ll-reserve]) and the capacity less the
loop's MIR pressure. Capacity is 6 general registers
([`register_capacity`][ll-capacity]). [`_formula_set`][ll-formula] collapses
sibling formulas into one shared index only when that budget is exceeded. The
backend then restricts a frame-array base to SI and DI
([`allocate::classes`][ll-frame]), a limit the budget never saw.

**Open Watcom.** [`MergeVars`][ow-merge] gives derived induction variables of
the same basic variable one register when they only differ by a
displacement. [`ReduceVar`][ow-reduce] performs the reduction. Its
addressing-mode check is disabled ([`IsAddressMode`][ow-addrmode]).

**LLVM.** [`LoopStrengthReduce.cpp`][lv-lsr] enumerates formulas for every
use, prices each by the target's legal addressing modes and register count,
and solves for the cheapest combination.

**Plan.**

1. Always consider the shared-index formula for siblings that differ by a
   constant, and prefer it when their bases share an addressing mode:
   `[bp+si+disp]`, or `[esi*8+disp]` under 32-bit addressing.
2. Budget by class: count SI and DI for frame-array bases, and BX, SI and DI
   for other word bases, instead of 6 interchangeable registers.

**Payoff.** Removes the spilled pointers and repeated `lea`s in array loops.
It is the second cause of `nbody`'s gap.

## 4. Address generation stalls

**Evidence.** On the 486 and P5, using a register as an address one
instruction after changing it costs a clock (AGI). On the 486, an
instruction with both a displacement and an immediate costs one more.

**llrm today.** [`schedule.rs`][ll-schedule] reorders only integer
register-to-register work. Memory operations, far calls, segment state and
x87 instructions are boundaries.

**Open Watcom.** An `add reg,const` may move past instructions that use `reg`
only as an index, adjusting their displacements ([`INS_INDEX_ADJUST`][ow-adjust],
[`FixIndexAdjust`][ow-fixadjust]). [`OkToSlide`][ow-slide] avoids the 486
displacement-plus-immediate clock, [`HiddenDependancy`][ow-hidden] catches
AL/AH partial-register dependencies, and [`StallCost`][ow-stall] prices
candidates against functional-unit tables.

**Plan.**

1. Give the scheduler memory dependencies for frame cells, the
   prerequisite for moving anything past a memory operation.
2. Port the index-adjust move: `add si,k` slides past uses of `si` as an
   address by rewriting their displacements.

**Payoff.** One clock per stall in pointer-walking loops on both CPUs.

## 5. x87 interleaving

**Evidence.** On the P5, `fxch` pairs for free, and independent floating
chains can overlap their 3-clock latencies. llrm emits `fxch` on demand and
never interleaves chains.

**Open Watcom.** On the P5, [`FPRegAlloc`][ow-586] numbers each independent
floating tree as its own sequence. The list scheduler interleaves sequences,
[`FPStkOver`][ow-overflow] guards stack depth, [`FPPreSched`][ow-presched]
and [`FPPostSched`][ow-postsched] map virtual to actual stack slots, and
[`GetToTopOfStack`][ow-top] emits `fxch` lazily. [`Opt87Sequence`][ow-opt87]
rewrites patterns such as `FLD ST; FSTP x` into `FST x`.

**LLVM.** [`X86FloatingPoint.cpp`][lv-fp] stackifies after scheduling, so
the scheduler sees virtual floating registers.

**Plan.** Only after item 1. Schedule across x87 instructions with virtual
stack slots, then stackify. The exception-ordering rule that makes x87 a
boundary today must hold across the new order.

**Payoff.** P5 floating throughput. Less on the 486, where the FPU is not
pipelined, although integer work could fill `fmul` and `fdiv` latency.

## 6. Register allocator

llrm's general allocator is already a port of LLVM's greedy allocator
([`allocate.rs`][ll-allocate]): priority queue ([`_priority`][ll-priority]),
eviction ([`_evict`][ll-evict]), a split stage, then spill. Neither Open
Watcom's allocator ([`RegAlloc`][ow-regalloc], [`GiveRegister`][ow-give]),
which is greedy without eviction and spills whole variables, nor gcc-ia16's,
which is GCC's generic LRA, is stronger. The remaining gaps are LLVM parts
not yet ported.

- *Split placement.* llrm splits coarsely: whole hot blocks
  ([`splitkit::_regional`][ll-regional]), one gap, or one block, once per
  value ([`split`][ll-split]). LLVM places region splits by cost
  ([`SplitKit.cpp`][lv-splitkit], [`SpillPlacement.cpp`][lv-spillplace]).
- *Spill hoisting.* llrm reloads before every use; only
  [`spillforward.rs`][ll-spillforward] removes redundant reloads afterwards.
  LLVM's [`InlineSpiller.cpp`][lv-spiller] hoists spills to the cheapest
  dominating point.
- *Last-chance recoloring* in [`RegAllocGreedy.cpp`][lv-greedy] re-assigns
  interfering values before giving up. llrm only accepts zero-cost evictions.
- *Incremental allocation.* The driver re-runs the whole allocation for every
  trial and round, up to 12 ([`ROUNDS`][ll-rounds]).

**Plan.** First measure spill traffic, meaning executed reloads and stores,
on the benchmarks. Port whichever of these accounts for most of it.

## 7. Smaller items

- **Branch-seeded equalities.** On the taken edge of `cmp a,b; je`, `a == b`
  ([`ScoreSeed`][ow-seed]). This belongs in llrm's copy propagation and
  machine CSE.
- **Post-allocation memory forwarding.** Replace loads of compiler-made frame
  cells with a register already holding them, using Open Watcom's
  invalidation rules ([`ScoreStomp`][ow-stomp]).
  [`machinecse.rs`][ll-machinecse] excludes memory reads by design.
- **Tail merging and cloning.** Merge identical code tails
  ([`FindCommon`][ow-common]) and duplicate short ones instead of jumping to
  them ([`CloneCode`][ow-clone]).
- **gcc-ia16 idioms**, to check against llrm's peepholes:
  - `x*257` as `add ah,al` ([`*mulhi3_const`][gi-mul257])
  - arithmetic `>>15` as `cwd` ([`*ashrhi3_const15`][gi-cwd])
  - `(x>>8)&255` as a byte move ([`*lshrhi_const8_andhi_const255`][gi-shr8])
  - rotate by 8 as `xchg al,ah` ([`*rotlhi3_const8`][gi-rot8])
  - the partial-flag lattice that lets a later jump reuse flags
    ([`ia16_cc_modes_compatible`][gi-cc])
  - `mov ax,r` as the one-byte `xchg` when `r` dies
    ([`ia16_rewrite_movw_as_xchgw`][gi-xchg])
- **Cost-table checks.** Open Watcom's `rep movs` versus unrolled `movs`
  thresholds ([`x86split.c`][ow-repmovs]) and avoiding `leave` on the 486
  ([`x86proc.c`][ow-leave]). Also check that llrm prices the operand- and
  address-size prefixes its 32-bit forms add.

## 8. Not worth porting

These pay on an 8086 or 286, not on a 486.

- **Segment registers as spare registers.** gcc-ia16 lets its move pattern
  place any 16-bit value in DS or ES ([`*mov<mode>`][gi-mov]). It restores DS
  before calls ([`ia16_expand_reset_ds_for_call`][gi-resetds]) and removes
  the restores it doesn't need ([`ia16_elide_unneeded_ss_stuff`][gi-elide]).
  On a 486, moving into a segment register costs about 3 clocks, where a
  cached spill costs 1.
- **32-bit values as register pairs**, runtime helpers for 32-bit multiply
  and divide, and loop-based 32-bit shifts, in Open Watcom and gcc-ia16. llrm
  uses 32-bit registers.
- **8086 addressing register classes** ([`REG_ALLOC_ORDER`][gi-order]). llrm
  models these restrictions already, and 32-bit addressing removes most of
  them.
- **8086-tuned cost tables** ([`IA16_COST`][gi-cost],
  [`processor_target_table`][gi-cputable]) and SS-override removal, which
  only saves size.

[ll]: https://github.com/ali-mosavian/llrm/tree/079fceb311cb4d530cf40e111a149df3f729e33b
[ow]: https://github.com/open-watcom/open-watcom-v2/tree/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6
[gi]: https://gitlab.com/tkchia/gcc-ia16/-/tree/9a5539544a9095ad35c4f57a23f2b79461032cb2
[lv]: https://github.com/llvm/llvm-project/tree/llvmorg-20.1.8

[bench]: https://github.com/ali-mosavian/llrm/tree/079fceb311cb4d530cf40e111a149df3f729e33b/bench/c
[expected]: https://github.com/ali-mosavian/llrm/blob/079fceb311cb4d530cf40e111a149df3f729e33b/bench/c/expected.json
[pinned]: https://github.com/ali-mosavian/llrm/blob/1ded74a4109f5bf41d7eddb10c779c552c84075b/conf/pinned.conf

[ll-promote]: https://github.com/ali-mosavian/llrm/blob/079fceb311cb4d530cf40e111a149df3f729e33b/crates/llrm-core/src/optimize/promote.rs#L1094
[ll-exact]: https://github.com/ali-mosavian/llrm/blob/079fceb311cb4d530cf40e111a149df3f729e33b/crates/llrm-core/src/optimize/transform.rs#L317
[ll-fenv]: https://github.com/ali-mosavian/llrm/blob/079fceb311cb4d530cf40e111a149df3f729e33b/docs/semantics/floating-environment.md
[ll-exactdoc]: https://github.com/ali-mosavian/llrm/blob/079fceb311cb4d530cf40e111a149df3f729e33b/docs/optimizations/exact-floating-store-reuse.md
[ll-boundary]: https://github.com/ali-mosavian/llrm/blob/079fceb311cb4d530cf40e111a149df3f729e33b/crates/llrm-core/src/backend/floatregions.rs#L67
[ll-bridged]: https://github.com/ali-mosavian/llrm/blob/079fceb311cb4d530cf40e111a149df3f729e33b/crates/llrm-core/src/backend/floatregions.rs#L84
[ll-refuse]: https://github.com/ali-mosavian/llrm/blob/079fceb311cb4d530cf40e111a149df3f729e33b/crates/llrm-core/src/backend/floatalloc.rs#L1204
[ll-retain]: https://github.com/ali-mosavian/llrm/blob/079fceb311cb4d530cf40e111a149df3f729e33b/crates/llrm-core/src/backend/floatalloc.rs#L622
[ll-room]: https://github.com/ali-mosavian/llrm/blob/079fceb311cb4d530cf40e111a149df3f729e33b/crates/llrm-core/src/backend/floatalloc.rs#L742
[ll-9a]: https://github.com/ali-mosavian/llrm/blob/079fceb311cb4d530cf40e111a149df3f729e33b/crates/llrm-core/src/backend/omfwrite.rs#L819
[ll-reduced]: https://github.com/ali-mosavian/llrm/blob/079fceb311cb4d530cf40e111a149df3f729e33b/crates/llrm-core/src/optimize/strength.rs#L133
[ll-candidates]: https://github.com/ali-mosavian/llrm/blob/079fceb311cb4d530cf40e111a149df3f729e33b/crates/llrm-core/src/optimize/strength.rs#L701
[ll-reserve]: https://github.com/ali-mosavian/llrm/blob/079fceb311cb4d530cf40e111a149df3f729e33b/crates/llrm-core/src/optimize/strength.rs#L282
[ll-formula]: https://github.com/ali-mosavian/llrm/blob/079fceb311cb4d530cf40e111a149df3f729e33b/crates/llrm-core/src/optimize/strength.rs#L1294
[ll-capacity]: https://github.com/ali-mosavian/llrm/blob/079fceb311cb4d530cf40e111a149df3f729e33b/crates/llrm-core/src/backend/cpu.rs#L24
[ll-frame]: https://github.com/ali-mosavian/llrm/blob/079fceb311cb4d530cf40e111a149df3f729e33b/crates/llrm-core/src/backend/allocate.rs#L392
[ll-schedule]: https://github.com/ali-mosavian/llrm/blob/079fceb311cb4d530cf40e111a149df3f729e33b/crates/llrm-core/src/backend/schedule.rs#L1
[ll-allocate]: https://github.com/ali-mosavian/llrm/blob/079fceb311cb4d530cf40e111a149df3f729e33b/crates/llrm-core/src/backend/allocate.rs#L724
[ll-priority]: https://github.com/ali-mosavian/llrm/blob/079fceb311cb4d530cf40e111a149df3f729e33b/crates/llrm-core/src/backend/allocate.rs#L951
[ll-evict]: https://github.com/ali-mosavian/llrm/blob/079fceb311cb4d530cf40e111a149df3f729e33b/crates/llrm-core/src/backend/allocate.rs#L1074
[ll-rounds]: https://github.com/ali-mosavian/llrm/blob/079fceb311cb4d530cf40e111a149df3f729e33b/crates/llrm-core/src/backend/allocate.rs#L1138
[ll-regional]: https://github.com/ali-mosavian/llrm/blob/079fceb311cb4d530cf40e111a149df3f729e33b/crates/llrm-core/src/backend/splitkit.rs#L216
[ll-split]: https://github.com/ali-mosavian/llrm/blob/079fceb311cb4d530cf40e111a149df3f729e33b/crates/llrm-core/src/backend/splitkit.rs#L51
[ll-spillforward]: https://github.com/ali-mosavian/llrm/blob/079fceb311cb4d530cf40e111a149df3f729e33b/crates/llrm-core/src/backend/spillforward.rs
[ll-machinecse]: https://github.com/ali-mosavian/llrm/blob/079fceb311cb4d530cf40e111a149df3f729e33b/crates/llrm-core/src/backend/machinecse.rs#L101

[ow-cache]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/intel/c/i87sched.c#L733
[ow-init]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/intel/c/i87sched.c#L849
[ow-fini]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/intel/c/i87sched.c#L829
[ow-between]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/intel/c/i87sched.c#L680
[ow-fpalloc]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/intel/c/i87reg.c#L680
[ow-another]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/intel/c/i87reg.c#L216
[ow-calls]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/intel/c/i87reg.c#L611
[ow-586]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/intel/c/i87reg.c#L498
[ow-overflow]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/intel/c/i87sched.c#L946
[ow-presched]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/intel/c/i87sched.c#L1018
[ow-postsched]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/intel/c/i87sched.c#L1075
[ow-top]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/intel/c/i87sched.c#L235
[ow-opt87]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/intel/c/i87opt.c#L558
[ow-cheap]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/intel/c/x86enc2.c#L226
[ow-outccyp]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/intel/c/x86esc.c#L300
[ow-retaftr]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/c/optpush.c#L46
[ow-comment]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/intel/c/x86omf.c#L711
[ow-farcallopt]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/wl/c/obj2supp.c#L1172
[ow-merge]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/c/loopopts.c#L1607
[ow-reduce]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/c/loopopts.c#L3320
[ow-addrmode]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/c/loopopts.c#L1657
[ow-adjust]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/c/inssched.c#L145
[ow-fixadjust]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/c/inssched.c#L759
[ow-slide]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/c/inssched.c#L246
[ow-hidden]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/c/inssched.c#L277
[ow-stall]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/c/inssched.c#L653
[ow-regalloc]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/c/regalloc.c#L1303
[ow-give]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/c/regalloc.c#L1139
[ow-seed]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/c/scmain.c#L61
[ow-stomp]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/c/scinfo.c#L72
[ow-common]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/c/optcom.c#L184
[ow-clone]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/c/optpull.c#L62
[ow-repmovs]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/intel/c/x86split.c#L269
[ow-leave]: https://github.com/open-watcom/open-watcom-v2/blob/703e1ae2f9a621dda2d28fb39b12d0d6d2788af6/bld/cg/intel/c/x86proc.c#L800

[gi-call]: https://gitlab.com/tkchia/gcc-ia16/-/blob/9a5539544a9095ad35c4f57a23f2b79461032cb2/gcc/config/ia16/ia16.c#L5914
[gi-mul257]: https://gitlab.com/tkchia/gcc-ia16/-/blob/9a5539544a9095ad35c4f57a23f2b79461032cb2/gcc/config/ia16/ia16.md#L1845
[gi-cwd]: https://gitlab.com/tkchia/gcc-ia16/-/blob/9a5539544a9095ad35c4f57a23f2b79461032cb2/gcc/config/ia16/ia16.md#L1997
[gi-shr8]: https://gitlab.com/tkchia/gcc-ia16/-/blob/9a5539544a9095ad35c4f57a23f2b79461032cb2/gcc/config/ia16/ia16.md#L2029
[gi-rot8]: https://gitlab.com/tkchia/gcc-ia16/-/blob/9a5539544a9095ad35c4f57a23f2b79461032cb2/gcc/config/ia16/ia16.md#L2117
[gi-cc]: https://gitlab.com/tkchia/gcc-ia16/-/blob/9a5539544a9095ad35c4f57a23f2b79461032cb2/gcc/config/ia16/ia16.c#L2638
[gi-xchg]: https://gitlab.com/tkchia/gcc-ia16/-/blob/9a5539544a9095ad35c4f57a23f2b79461032cb2/gcc/config/ia16/ia16-reorg.c#L1192
[gi-mov]: https://gitlab.com/tkchia/gcc-ia16/-/blob/9a5539544a9095ad35c4f57a23f2b79461032cb2/gcc/config/ia16/ia16.md#L266
[gi-resetds]: https://gitlab.com/tkchia/gcc-ia16/-/blob/9a5539544a9095ad35c4f57a23f2b79461032cb2/gcc/config/ia16/ia16.c#L5674
[gi-elide]: https://gitlab.com/tkchia/gcc-ia16/-/blob/9a5539544a9095ad35c4f57a23f2b79461032cb2/gcc/config/ia16/ia16-reorg.c#L956
[gi-order]: https://gitlab.com/tkchia/gcc-ia16/-/blob/9a5539544a9095ad35c4f57a23f2b79461032cb2/gcc/config/ia16/ia16.h#L119
[gi-cost]: https://gitlab.com/tkchia/gcc-ia16/-/blob/9a5539544a9095ad35c4f57a23f2b79461032cb2/gcc/config/ia16/ia16.c#L2902
[gi-cputable]: https://gitlab.com/tkchia/gcc-ia16/-/blob/9a5539544a9095ad35c4f57a23f2b79461032cb2/gcc/config/ia16/ia16.c#L3481

[lv-fp]: https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Target/X86/X86FloatingPoint.cpp
[lv-lsr]: https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp
[lv-greedy]: https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp
[lv-splitkit]: https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/SplitKit.cpp
[lv-spillplace]: https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/SpillPlacement.cpp
[lv-spiller]: https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/InlineSpiller.cpp

# LIR peephole and x87 passes against LLVM's selection combine and stackifier

TL;DR: on `d_faces` `-O2` llrm's `lir peephole` costs 129.7 M instructions and its two x87 passes 70.9 M; LLVM does the same kind of work in DAG combining inside instruction selection (20.1 M of the 51.3 M) plus a 1.0 M `PeepholeOptimizer` and a 1.4 M `X86FPStackifier`. The peephole is about 40 whole-body sub-passes in a row, none above 10% of its cost; both x87 passes solve whole-function register liveness again (25.7 M of the 70.9 M), where LLVM's stackifier reads the live-in masks already on each block. Both are linear in the body; the ratio is a constant.

## LLVM

The rewrites a post-allocation peephole makes are made while the instruction is selected: the DAG combiner runs four times over each block's DAG (before type legalization, after it, after vector legalization, after legalization), each time over nodes whose known bits and use counts the DAG already holds. The first run is shown; the others are at [`#L1028`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGISel.cpp#L1028), [`#L1082`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGISel.cpp#L1082) and [`#L1122`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGISel.cpp#L1122).

[`SelectionDAGISel.cpp#L977-L983`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGISel.cpp#L977-L983)

```cpp

  // Run the DAG combiner in pre-legalize mode.
  {
    NamedRegionTimer T("combine1", "DAG Combining 1", GroupName,
                       GroupDescription, TimePassesIsEnabled);
    CurDAG->Combine(BeforeLegalizeTypes, getBatchAA(), OptLevel);
  }
```

The x87 stack is not allocated by the stackifier. Greedy allocation gives each x87 value one of seven flat registers like any other class; the stackifier then maps flat registers to stack positions, block by block, from the live-in lists the allocator left on each block, and keeps one 8-bit mask per edge bundle.

[`X86FloatingPoint.cpp#L81-L105`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FloatingPoint.cpp#L81-L105)

```cpp
  struct LiveBundle {
    // Bit mask of live FP registers. Bit 0 = FP0, bit 1 = FP1, &c.
    unsigned Mask = 0;

    // Number of pre-assigned live registers in FixStack. This is 0 when the
    // stack order has not yet been fixed.
    unsigned FixCount = 0;

    // Assigned stack order for live-in registers.
    // FixStack[i] == getStackEntry(i) for all i < FixCount.
    unsigned char FixStack[8];

    LiveBundle() = default;

    // Have the live registers been assigned a stack order yet?
    bool isFixed() const { return !Mask || FixCount; }
  };

  // Numbered LiveBundle structs. LiveBundles[0] is used for all CFG edges
  // with no live FP registers.
  SmallVector<LiveBundle, 8> LiveBundles;

  // The edge bundle analysis provides indices into the LiveBundles vector.
  EdgeBundles *Bundles = nullptr;

```

The live registers of a block are read from its live-in list, not computed:

[`X86FloatingPoint.cpp#L107-L120`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FloatingPoint.cpp#L107-L120)

```cpp
  static unsigned calcLiveInMask(MachineBasicBlock *MBB, bool RemoveFPs) {
    unsigned Mask = 0;
    for (MachineBasicBlock::livein_iterator I = MBB->livein_begin();
         I != MBB->livein_end();) {
      MCPhysReg Reg = I->PhysReg;
      static_assert(X86::FP6 - X86::FP0 == 6, "sequential regnums");
      if (Reg >= X86::FP0 && Reg <= X86::FP6) {
        Mask |= 1 << (Reg - X86::FP0);
        if (RemoveFPs) {
          I = MBB->removeLiveIn(I);
          continue;
        }
      }
      ++I;
```

## llrm

`Peephole::once` is a fixed sequence of about 40 sub-passes. Each takes the body and returns a new one; the generated rule groups walk every instruction of every block.

[`peephole.rs#L314-L330`](https://github.com/ali-mosavian/llrm/blob/21f5c9fc8e67b5bf405cf9ecdaf65247a26c0450/crates/backend/llrm-core/src/backend/peephole.rs#L314-L330)

```rust
        // Before the rest: a thrashed copy is one fewer instruction for
        // everything below to reason about, and it is the only pass here
        // that can remove a copy the coalescer refused on colourability.
        let mut seen = needs::Contents::default();
        let body = gated(&mut seen, needs::COPY, phielim::unsplit(&body), |body| Ok(regthrash::thrashed(body)))?;
        let body = frame_copies(&body, &self.cpu, &self.classes)?;
        let body = gated(&mut seen, needs::COPY, body, |body| Ok(copyprop::forwarded(&body)))?;
        let body = extensions(self.rules, &body);
        let body = copysink::sunk(&body);
        let body = spillforward::forwarded(&body);
        // After the forwarding: a reload on the other path is gone, and the
        // store need not run there.
        let body = crate::backend::postrasink::latched(&crate::backend::postrasink::sunk(&body));
        let body = storecombine::combined(&body);
        let body = pushed_constants(self.rules, &body);
        let body = far_loads(
            self.rules,
```

A rule group's walk builds a window for every instruction: a `Vec` of the instruction positions and a `Window` over it (43,539 allocations in 16.4 M on `d_faces`).

[`walk.rs#L599-L615`](https://github.com/ali-mosavian/llrm/blob/21f5c9fc8e67b5bf405cf9ecdaf65247a26c0450/crates/backend/llrm-core/src/backend/peep/walk.rs#L599-L615)

```rust
fn windows(
    cx: &Cx,
    matcher: &Matcher,
    skip: Skip,
) -> Option<Vec<Arc<Insn>>> {
    let insns = cx.insns;
    let code: Vec<usize> = (0..insns.len()).filter(|at| !skip.skips(&insns[*at])).collect();
    let mut out: Option<Vec<Arc<Insn>>> = None;
    let mut retired = Vec::new();
    let (mut next, mut at) = (0, 0);
    while at < code.len() {
        if let Some(out) = &mut out {
            out.extend(insns[next..code[at]].iter().cloned());
        }
        let taken = code[at..(at + matcher.width).min(code.len())].to_vec();
        match run(cx, matcher, &Window::new(insns, taken, &[])) {
            Some(made) => {
```

`floatassign` is an allocator for the x87 registers (allocnos priced against memory, Belady spills); after each victim it asks which floating values are live across each edge bundle, as rows over the whole body. `floatalloc` then converts to stack form and solves liveness again.

[`floatassign.rs#L1866-L1869`](https://github.com/ali-mosavian/llrm/blob/21f5c9fc8e67b5bf405cf9ecdaf65247a26c0450/crates/backend/llrm-core/src/backend/floatassign.rs#L1866-L1869)

```rust
    let registers = body.float_stack;
    // Only the floating values are followed, as rows: a set of every live value
    // of every block was built for each victim spilled (`fjoins` 512: 100 G).
    let rows = crate::backend::allocate::live_rows_by(body, |value| floating.contains(&value));
```

## Divergence

Selection, DAG combine and the post-RA passes together are 20 + 8 + 1.0 + 3.2 + 2.7 M in LLVM against 54.6 + 129.7 M in llrm for selection and `lir peephole`: llrm pays for a second full pass over the allocated body per rule group, with the lane and flag facts of each (`liveness::dead_at_exit` 5.5 M, `allocate::live` 3.9 M, `upper_zero` 14.7 M, `Facts::new` 44 times) re-derived by the sub-passes that need them. No single sub-pass is the cause; the sum of about 40 is. The x87 pair is two different things: `floatassign` (47.3 M) allocates registers, which in LLVM is part of greedy allocation (39.5 M for every class), and `floatalloc` (23.7 M) is the stackifier's counterpart (1.4 M, 17x). `floatalloc` and `floatassign` each ask for the whole body's live rows five times on `d_faces` (12.9 M and 12.8 M), where the stackifier asks nothing: the allocator has left the live-in lists on the blocks.

The roadmap already names this: step 6 of [compile-time spec](../../architecture/compile-time-spec.md), "LIR liveness owner with dirty blocks; peephole and jumps on dirty blocks; floatassign exits before rebuilding", and the peephole solves a function's lanes once (`test_the_peephole_solves_a_functions_lanes_from_scratch_once`).

## Measured

Instructions: `valgrind --tool=callgrind --inclusive=yes` on `llrm-c` of main `21f5c9fc8` and on the driver's `cc1` for LLVM; `LLRM_DEBUG=time` for the ladder. `d_faces.c` of qbq-llrm, `-O2 -m32 -march=i486`. The output is 1,835 instructions in llrm and 1,863 in clang, so a per-instruction rate means the same thing on both sides.

| step | LLVM | llrm |
|---|---|---:|
| rewrites at selection | `X86DAGToDAGISel` 51.3 (of it `SelectionDAG::Combine` 20.1, `DoInstructionSelection` 8.3) | `isel` 54.6 |
| post-allocation clean-up | `PeepholeOptimizer` 1.0, `MachineCopyPropagation` 3.2, `MachineCSE` 2.7 | `lir peephole` 129.7 |
| x87 register choice | inside `RAGreedy` 39.5 (all classes) | `lir floatassign` 47.3 |
| x87 stack form | `X86FPStackifier` 1.4 | `lir floatalloc` 23.7 |

Inside `lir peephole` (11 bodies, inclusive M, calls in brackets):

| sub-pass | M | | sub-pass | M |
|---|---:|---|---|---:|
| `widened_moves` and `zero_extensions` rule groups | 13.6 (44) | | `upperzero::hoisted` | 5.2 (11) |
| `copyprop::forwarded` | 13.6 (6) | | `increments` | 5.1 (11) |
| `exact_addresses` (gated) | 9.1 (11) | | `machinecse::eliminated` | 4.2 (11) |
| `spillforward::forwarded` | 8.4 (11) | | `liveness::dead_at_exit` | 5.5 (23) |
| `machinedce::eliminated` | 6.1 (11) | | `allocate::live` | 3.9 (11) |
| `fused` | 5.9 (11) | | `peephole::zeroes` | 3.0 (11) |
| `regthrash::thrashed` | 5.9 (6) | | the rest | 0.3 to 2.1 each |
| `narrowed_arithmetic` | 5.6 (11) | | | |

Inside `lir floatassign` (4 bodies with floats): `_allocnos` 18.1, `allocate::live` 12.8 (5 calls), `Plan::rewritten` 5.7, `_crowded` 4.4. Inside `lir floatalloc`: `allocate::live` 12.9 (5 calls), the conversion closure 3.3.

Slope, `LLRM_DEBUG=time` (M instructions, N = 100, 400, 1600 statements): `lir peephole` `arith` 14.7, 59.7, 216.4; `mem` 10.9, 26.4, 107.0; `diamond` 12.4, 58.0, 254.7; `calls` 11.4, 39.8, 154.5 (exponent 0.9 to 1.0). The `float` shape of `tools/ladder/ladder_gen.py` (arithmetic on four `double`s): `floatassign` 4.9, 18.7, 69.0 and `floatalloc` 1.0, 4.6, 18.1, against LLVM's whole `llc` of 21, 92 and 455 M and its stackifier at 2.0 M (N=400). Linear too: the x87 passes are 11.6x the stackifier on straight-line floats at N=400, 17x on `d_faces`, where control flow makes the live rows bigger.

## Facts the peephole derives, and what one table would remove

Counted from the same callgrind run (11 bodies; the output has 1,835 instructions, so a count per instruction below is an upper bound on repeats, the input to the peephole being larger):

| fact | derived by | derivations | cost (M) | derived once per body would cost |
|---|---|---:|---:|---|
| lanes an instruction reads and writes (per-`Insn` `OnceLock`) | `liveness::worked_out`, `spillforward::_transfer`, `copyprop::forwarded_inner`, `once` and four more | 8,140 (at most 4.4 per instruction) | 15.1 | at least 3.4 M: 1,835 derivations of the 8,140 |
| lane liveness at block exits (`dead_at_exit`) | `machinedce` 27, `once` 23, `regthrash` 6 | 56 (5.1 per body) | 21.7 | 4.3 M: 11 solves of the 56 |
| upper half of a register proven zero (`upper_zero`) | the `widened_moves`, `zero_extensions` and `narrowed_arithmetic` groups | 6 (under once per body) | 14.7 | 14.7 M: it is derived lazily and rarely; each derivation costs 2.45 M |
| flag liveness (`_flags_dead_after`, `_flags_live_out`, `regthrash::_dead_after`) | `tested`, `zero_compares`, `regthrash` | 1,216 queries | 5.9 | per query, not per body |
| whole-function live rows (`allocate::live`) | `once` | 11 (once per body) | 3.9 | 3.9 M: already once per body |

The overlap between rows 1 and 2 is 5.7 M (`worked_out` is inside `dead_at_exit`), so the facts cost about 55.6 M, 43% of the peephole and 1.9% of `d_faces` -O2. One table that held each of the two repeated facts once per body would remove at most 11.7 M of lane effects and 17.4 M of block-exit liveness, 29 M in all: 22% of the peephole, 1.0% of `d_faces`. The rest of the fact cost is not repetition: `upper_zero` is derived fewer than once per body and costs 2.45 M a time. These are scaled from call counts and are not measured by running a shared table.

## Proposed change

For the backend owner (ssa-spill-m16); none built here.

1. One owner for the peephole's lane and block-exit facts (rule 7): at most 29 M, 1.0% of `d_faces` (sizing above). Dropped by the coordinator as not worth a PR.
2. Take the window as a slice: `taken` in [`windows`](https://github.com/ali-mosavian/llrm/blob/21f5c9fc8e67b5bf405cf9ecdaf65247a26c0450/crates/backend/llrm-core/src/backend/peep/walk.rs#L612) is a `Vec` built per instruction, 43,539 allocations. Bound: about 4 M at 100 instructions per allocation (2.8 M of allocator calls seen in `windows`). Saving: unmeasured.
3. Gate the rule groups the way `needs` gates six sub-passes: `needs.rs` keeps one bit per candidate shape per block. The idle-skip experiment in the spec skipped 6% of MIR runs and lost 0.6% overall, because classifying costs; a bit per block set when an instruction of the shape is produced is cheaper than classifying edits. Bound: the sub-passes that find nothing; their number is not counted. Saving: unmeasured.
4. Pass the live rows `regalloc` already solved to `floatassign` and `floatalloc` (or hold them on the body) instead of `live_rows_by` per call. Bound: 25.7 M (0.9% of `d_faces`); ranks last. Saving: unmeasured.
5. Do at selection what the peephole does after allocation where the rewrite needs no physical register: LLVM's four combine runs cost 20.1 M for the same file. Design, not a patch; the sub-passes that need final registers (flags, lanes) stay.

Open: how many of the 35 sub-passes change nothing on a typical body, and what `upper_zero` (14.7 M, 6 calls) is computed for on a 32-bit target ([#1994](https://github.com/ali-mosavian/llrm/issues/1994)).

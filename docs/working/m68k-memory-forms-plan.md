# m68k memory forms after 2c — plan (llrm-target-m68k, 2026-11-01)

Gap to best of gcc/clang, 22 programs, -O2/-Os, tip of #6324 (greedy + 2a/2b/2c): ours 1008 B, best 876 B (+132; was +176 before 2c).
The base-update merge I was asked to plan (item 3) is not where the bytes are. Measured, from the listings:

| Cluster | Programs (B behind) | What gcc does |
|---|---|---|
| A. load not folded into its user | arrmax 10, countgt 10, arrsum 8, copy 4, sum 2 | `add.l (%a0)+,%d0`, `cmp.l (%a2)+,%d0`: one insn, one register fewer (ours also saves/restores d2 or a2) |
| B. loop counter kept beside the pointer | crc16 18, partition 8, hex part | pointer compared with an end pointer; no counter register |
| C. dead base, offset chain | structcopy 16, memcpy 6 | `move.l (%a0)+,(%a1)+` x3, no displacement words |
| D. call arguments | muldiv 14 | `move.l x,-(%sp)` pushes, no frame store |
| E. pre-decrement | hex 18 | `-(%a0)` stores |
| Block-local `(An)` + `addq` merge (the asked item) | 3 executed sites over 22 programs (strcmp, memset, 16 insns) | already `(An)+` almost everywhere via the DAG combine |

## Order and mechanism (each a general rule, none keyed to a program)

1. **A: fold a single-use load into its user.** Translate `PeepholeOptimizer::optimizeLoadInstr` + `TargetInstrInfo::foldMemoryOperand`, pre-RA on the machine function. The pin's m68k has no `foldMemoryOperandImpl`; DEVIATE, exit none. The pin's ALU rows have `dj`/`dp` (memory source) but no post-increment form, so add writeback ALU opcodes the way `MOV32do_WB` is built (`indexed.rs`: description derived from the ARI row, tied base, prints as `(An)+`): ADD/SUB/AND/OR/CMP x B/W/L. Data: which pin rows have a memory twin comes from the operand classes, not a list. Frees a register and an `move` per load; the d2/a2 save disappears when pressure drops. Two readers: binutils encodings (tests/short.rs style) and qemu values.
2. **C: dead base chain.** Accesses at offsets 0, s, 2s ... from a base that is dead after the last one become `(An)+` (any order the offsets allow). gcc's `auto_inc_dec` finds it after address arithmetic is combined; LLVM has nothing. Pass on machine IR after RA, liveness from the existing analysis. Cost-priced by `timing::cost` both ways (68000 and 68020).
3. **Base-update merge** (the original item): translate `AArch64LoadStoreOpt::findMatchingUpdateInsnForward/Backward` + `mergeUpdateInsn`, post-RA, block-local, `UpdateLimit` kept. Small on this corpus; lands after 1-2 so it is measured on what remains. It also covers `-(An)` and the extending load.
4. **B: IV elimination against an end pointer.** This is `LoopStrengthReduce` formula choice. The pin has no m68k TTI, so `isLSRCostLess`/`shouldFavorPostInc` use the defaults. A target-description change (one TTI answering "post-increment is free", "register count 8+8"), checked by the loop-reduce unit tests in the pin, before any pass. Also decides E. Prove on `opt` output at the pin first (POC, like the other structural changes).
5. **D: outgoing arguments as pushes.** Frame lowerer (reorg-layout / `callframe` owners): hand over with the muldiv listing, do not edit.

## Per-item gates (oversight's list)
DEVIATE row, descriptors as data with the two-reader check, encodings vs binutils 2.45, `-Os` never larger per program on 68000 and 68020, 300-seed random static sweep, old-bar and best-of table, fail-first test per form, draft+WIP PR until evidence, shared-trait changes with a live-control object sweep.

## Questions
- Item 1 first, then 2, then 3, 4? Or 4 (POC only) in parallel since it is a read-only pin experiment?
- D: who owns the outgoing-argument push form?

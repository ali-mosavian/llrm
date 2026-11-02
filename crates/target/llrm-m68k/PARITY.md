# m68k: parity with the pin

The default output is the pin's `llc` output: `Options::PINNED` is checked against `llc`
(`tests/pin_output.rs`, `examples/bench.rs`), and `examples/difftest.rs` runs random C programs
under `qemu-m68k` against `m68k-linux-gnu-gcc` (15.2.0, Ubuntu 15.2.0-4ubuntu4; it fails without
one and prints the driver and version).

## Deviations, numbered

| # | Where | Deviation from the pin | Exit |
|---|---|---|---|
| 1 | `expand.rs` | `ExpandMOVI` keeps `isInt<16>` for `movea.w` (the pin tests `isUInt<16>`, which sign-extends 32768..65535 wrongly) | the pin's test is fixed upstream |
| 2 | `expand.rs` | only 32-bit copies, `moveq` immediates and zero-extending byte loads expand; no 16-bit destinations | M4c, or when a kernel needs them |
| 3 | `mfunc.rs` | `MOVZX`/`MOVSX` results and sources are data registers (the pin lets an address register through and then mis-encodes it) | never; a fix the pin lacks |
| 4 | `mfunc.rs` | other readers of a compare's codes are hoisted above the select diamond, a second `CMOV` on the same codes saves them in the head and restores them in the join (`CCR_SAVE`/`CCR_RESTORE`); the cascaded `CMOV (CMOV ..)` form is not expanded | the first never; the cascade when a kernel needs it |
| 5 | `flags.rs` | codes clobbered by a copy are re-issued (`cmpi #0` or the same compare) instead of the pin's `move %sr` save/restore | where neither re-issue works |
| 6 | `regalloc.rs` | `split_flag_edges`: a critical edge into a phi gets its own block when the copy would sit above the compare (`SplitPHIEdges` splits fewer); `a_max_loop_carries_its_maximum_in_three_register_moves_at_most` fails without it (arrmax +2 B, +24 insns). Moves in the max loop: spike 2, greedy 3, the pin 3 (`llvm-build-m68k/bin/llc` 24.0.0git `-mtriple=m68k -mcpu=M68000 -O2 -disable-lsr`, output md5 1c593995e6caf200eff377cc79c28329, IR in tests/data/max-loop.ll) | never; a target choice |
| 7 | `divide.rs` | `DIVL` encodings written as the hardware takes them | the pin's row is fixed upstream |
| 8 | `dagisel/addr.rs` | an index beside a frame-index base is not set (the pin stores `a[i]` of a stack array to `a[0]`) | never; a fix the pin lacks |
| 9 | `dagisel/predicates.rs` | `Mxstoreloadi*_safe` checks that the load does not extend | the pin's fragments check the type |
| 10 | `dagisel/lowering.rs` | an 8/16-bit multiply with overflow is the wide multiply (the pin reports zero overflow) | never, while the pin does |
| 11 | `dagisel/call.rs`, `frame.rs`, `lowering.rs` | no `inreg`, `nest`, `byval`, `sret`, varargs, tail calls, realignment, base pointer, dynamic allocas, `trunc` forms of `LowerToBTST`, `BR_JT`/`BR_CC`/`VAARG`/atomics | M3b, M3d |
| 12 | `dagisel/pipeline.rs`, `function.rs` | no `PerformDAGCombine`, no `PreprocessISelDAG`, no jump tables | M4 |
| 13 | `dagisel/call.rs` | the global node carries no target flag | S2 |
| 14 | `mc.rs`, `encode.rs` | sub-forms of the printer and the FPU encodings (`encodeFPSYSSelect`, `encodeScale`, `encodeIndexSuppress`) | M4b, M7 |
| 15 | `mfunc.rs`, `object.rs` | spike-local machine function and static-executable writer; every `ObjectTarget` format answers `Unsupported`; no 16->32 branch relaxation | neutral-lir and the object refactor |
| 16 | `mac.rs` | the Mac resource layout follows the book and is not run (no emulator or ROM) | when a ROM is fetched |
| 17 | `dagisel/indexed.rs`, `lowering.rs` | `(An)+` and `-(An)` accesses (the shared indexed combine, on) with m68k-local writeback opcodes (value and new base, base tied). The pin models them with one def and no tied base, its `SelectARIPI`/`SelectARIPD` never match and it sets no indexed action. Off for `Options::PINNED`. Loads that extend are not indexed | none: the goal is best of gcc and clang |
| 18 | `frame.rs`, `expand.rs` | the saved registers are pushed and popped (`move.l %d2,-(%sp)`, `movem.l regs,-(%sp)`) in place of the pin's `suba.l` and `movem` into slots, where the prologue and the epilogue are each no larger and no slower on the CPU (encoder bytes, timing tables); not with a frame pointer. Off for `Options::PINNED` | none: the goal is best of gcc and clang |
| 19 | `encode.rs` | `MOVM*em` (`movem` to `-(An)`) takes the mask in printed order and encodes it reversed (`movem.l %d2-%d3,-(%sp)` is `48E7 3000`, as the pin's `llvm-mc` gives); the pin's table takes it pre-reversed and prints it forward | none: the pin's encoding is wrong for hardware |
| 20 | `dagisel/short.rs`, `local.rs` | `addq`, `subq` and `tst` (a compare with zero) (local opcodes with the pin's `ADD*`/`SUB*` immediate rows' operands, flags and classes; text and encoding built from them) for an add or subtract of 1 to 8 to a data or address register or to memory, and of -8 to -1 as the other where nothing reads the carry; selected after the generated selector (`PostprocessISelDAG`). The pin has no `ADDQ`/`SUBQ`/`TST`. Off for `Options::PINNED` | none: the goal is best of gcc and clang |
| 21 | `cmpelim.rs`, `ccr.rs`, `isa/ccr.tsv` | a `tst` of a register the instruction before it defined is dropped when that instruction leaves the codes the following readers test as `tst` of its result would (flag effects from the manual's tables, checked under qemu). The pin has no `optimizeCompareInstr` for m68k. Off for `Options::PINNED` | none: the goal is best of gcc and clang |
| 22 | `addrarith.rs`, `negadd.rs` | a pointer stepped by a constant is added to in an address register (`addq`/`subq` 1..8, `lea (d16,An)`, else `adda`, by clocks then bytes from the timing tables, bytes first at -Os) instead of two copies around a data-register add; an `add #-n` is `subq #n` where the readers of the codes test only N and Z. The pin keeps the add in a data register and splits with its greedy allocator. Off for `Options::PINNED` | the greedy allocator makes the class choice itself |
| 23 | `foldload.rs`, `dagisel/indexed.rs` | a `(An)+` load whose value one arithmetic or compare instruction reads is that instruction's memory source (`add.l (%a1)+, %d0`); `ADD`/`SUB`/`AND`/`OR`/`CMP` get `(An)+` twins built from their `(An)` rows (mode field and printer only); a compare of the loaded value on the destination side swaps its operands and the conditions that read it. The pin folds only plain loads at selection and has no `foldMemoryOperandImpl`. Off for `Options::PINNED`  ; a load stored at once is one move from `(An)+` to the store's place (`MOV32po_WB` and kin, the pin's `MOV*o` rows with the updated bases) | none |
| 24 | `chain.rs`, `dagisel/function.rs` | accesses through one address register that step by their size are `(An)+` (the pin's `MOV*` row with the mode letter `o`), later accesses through it take the displacement less the chain; priced on the CPU, clocks then bytes (bytes first at `-Os`); an inline `memcpy`'s pairs are in address order (`ordered_copies`). The pin's `CombineToPostIndexedLoadStore` refuses a base whose other users fold into displacements, and M68k sets no indexed addressing. Off for `Options::PINNED` | none |
| 25 | `narrow.rs` | `and`, `or` and `eor` of a long immediate whose upper half is all ones (`and`) or zero (`or`, `eor`) is the word row on the same register when no instruction reads the codes after it (4 bytes for 6, 8 clocks for 16 on the 68000). The pin keeps the 32-bit constant; gcc's `iorsi3`/`xorsi3` templates print `.w`. Off for `Options::PINNED` | none |

Items 4 to 6, 17, 18 and 19 are the only places the code differs from the pin's output on purpose; the rest is
unfinished translation.

## What the removed overlay bought, per piece

Until this branch the default output beat the pin through post-selection rewrites ("overlay"). They
are deleted; each deletion is one commit with a pin-form test. Totals over the 22 bench programs
(`bench/kernels`, `bench/library`), default / `-Os` where they differ, from `examples/cycles.rs` at
each commit and its parent (68000 and 68020 clocks from `src/timing.rs`; qemu supplies the trace).

| piece | rows moved | bytes | insns | 68000 clk | 68020 clk |
|---|---|---|---|---|---|
| `lea` on data registers | 0 of 112 | +0 | +0 | +0 | +0 |
| indexed displacement pricing + reassociation | 2 of 112 | +0 / +2 | +0 / +14 | +0 / +56 | +0 / +28 |
| `boolean_by_negation` | 2 of 112 | +2 / +0 | -16 / +0 | +96 / +0 | +32 / +0 |
| post-increment cluster (M68K-PI) | 40 of 112 | +48 / +46 | +150 / +139 | +1486 / +1218 | +455 / +333 |
| `merge_masks`, `constant_into_destination`, `narrow_immediates` | 20 of 112 | +12 | +16 | +568 | +222 |
| test/compare reuse (no `optimizeCompareInstr` in the pin) | 32 of 112 | +14 / +16 | +25 | +120 / +150 | +50 / +65 |
| `addq`/`subq`/`tst`/`cmpa` rewrite + `widen_compare_operands` | 86 of 112 | +186 / +190 | +0 / +2 | +4146 / +4200 | +1972 / +1953 |
| `mark_commutable` | 16 of 112 | +4 | +10 | +44 | +22 |
| argument-slot load fold | 14 of 112 | +0 / +30 | +0 / +12 | +0 / -800 | +0 / -390 |
| `address_arithmetic` | 36 of 112 | +86 | +303 | +1340 | +692 |
| `load_as_source`, `ordered_copies` | 6 of 112 | +2 / +0 | +16 / +0 | +96 / +0 | +32 / +0 |
| push/pop frame, phi rotation, `Options::overlay` | 80 of 112 | +216 | +24 | +540 | +208 |
| `moveq`+`not.w`/`swap` constants, argument rematerialization | 4 of 112 | +4 | -1 | +4 | +2 |
| **all, first state to head** | 112 | +574 / +606 | +527 / +544 | +8440 / +7320 | +3687 / +3135 |


Every bench program is at or under `llc` in bytes at both levels except `bsearch` at `-Os` (106 B against 102). The 68020 column is the cache case of MC68020UM
section 8; the manual says its tables bound the real time and do not predict it.

## Open row

**M68K-PI: post/pre-increment addressing.** Exit: build it the LLVM way after the merge, not as a
text rewrite: `setIndexedLoadAction`/`setIndexedStoreAction`, `getPostIndexedAddressParts`, and
LSR's TTI preferred addressing mode. Cost today: 40 of 112 rows;
worst memcpy 36 -> 48 B and 50 -> 90 insns (508 -> 906 clk on the 68000), structcopy 34 -> 54 B.

**M68K-OS-1: `bsearch` at `-Os` is 4 B over `llc -Os`** (106 against 102). Not yet diagnosed; the
`size` flag of `place_blocks` is the first place to look. Exit: after the merge.

## Instruments

- Clocks: `src/timing.rs`, MC68000UM section 8 (16-bit timing; section 7 is the 8-bit MC68008) and
  MC68020UM section 8 (cache case). `tests/timing.rs` checks cells with table and page; the
  68000 move formula `4 + source + destination` matches all 216 cells of Tables 8-2 and 8-3.
  Nothing in the compiler calls it. Manuals: NXP `MC68000UM.pdf` sha256 `89b690b1923f8a3c...`,
  `M68020UM.pdf` sha256 `883cae47d3741c68...` (the `MC68020UM.pdf` URL is a 404; same manual).
- `examples/cycles.rs` disassembles with binutils (the pin's `llvm-objdump` shows `addq` and `tst`
  as `<unknown>`), prices each executed instruction, and marks `*` a data-dependent term at its
  bound and `?` an instruction the tables do not cover.
- `examples/difftest.rs`: random C against `m68k-linux-gnu-gcc` 15.2.0 under `qemu-m68k`; at head
  300 seeds from 2000 and 16 reductions agree.

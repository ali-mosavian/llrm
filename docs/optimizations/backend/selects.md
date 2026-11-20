> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/selects.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/selects.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/selects.rs sha256:86883299e87f9a33fe7ab813b0f80648a6d07db9bbdbd5ba11eb5f0a5b9513b1 -->

# selects

Turns each MIR `select` into a branch around an empty block and a phi, where
the target has no conditional move.

## How it works
`select c, a, b` splits its block at the select. The instructions after it
move to a new join block, which takes over the block's way out (the phis of
its successors are renamed to name the join). A new block `taken` holds only
a jump to the join. The original block ends in a branch on `c` to `taken` or
the join, and the join starts with `phi [a, taken], [b, block]`, which
replaces the select's uses:

```text
 block:  ...                   block:  ...
         %r = select c, a, b           br c, taken, join
         rest                  taken:  br join
                               join:   %r = phi [a, taken], [b, block]
                                       rest
```

DEVIATE S3 (no number): `llvm.usub.sat` is expanded here, before selection,
to `umax(a, b) - b`. The pin expands it in the DAG legalizer
(`TargetLowering::expandAddSubSat`, `TargetLowering.cpp`) to a subtraction
and a conditional move. Exit: the legalizer's expansion lands.

DEVIATE S4 (no number): `llvm.{u}{add,sub}.with.overflow` is expanded here
to the operation and `icmp ult`, for the selectors that have no UADDO
(`--isel=old`, shadow): CodeGenPrepare forms the intrinsic on every x86 width
(`shouldFormOverflowOp`), and the old selector stopped at
"@llvm.uadd.with.overflow.i16" (nib and qb tests). The pin's legalizer expands
UADDO the same way (`TargetLowering::expandUADDSUBO`). Exit: the old selector
is deleted.

DEVIATE S5 (no number): `llvm.experimental.noalias.scope.decl` is erased
here for the same selectors: no code runs it, and the DAG builder returns
at it (`SelectionDAGBuilder`, `case Intrinsic::experimental_noalias_scope_decl`).
Exit: the old selector is deleted.

DEVIATE S2 (no number): the pin keeps the select to instruction selection.
CodeGenPrepare's `optimizeSelectInst` forms no branch where
`isPredictableSelectExpensive` is false (in-order i486, `CodeGenPrepare.cpp`
`isFormingBranchFromSelectProfitable`), and `X86TargetLowering::
EmitLoweredSelect` then expands the select to a diamond on machine IR, with
both operands computed before it. Ours expands in MIR. The operand sinking
of `sinkSelectOperand`, which the pin does not do here, is deleted: it
moved nothing over 834 objects. Exit: the expansion at selection, the
registry slot `emit-lowered-select` (lir-cleanup-port, PR by 2026-11-13;
##3052).

Rows opened by the i64 select moving to the DAG (owner w12, 2026-11-09;
each cell is at or under the pin llc, over the previous MIR expansion;
bytes before -> after, release/next only):
- I64SEL-1, exit 2026-11-12: m32 `select_of_constants` .text +32 (-O1, -O2)
  and +64 (-O3), 12 of 420 functions grow by 1..7 B. Harness (run_, cycle
  model) clocks base -> head: O1 270645 -> 273325 (+1.0%), O2 267647 ->
  270127 (+0.9%), O3 266972 -> 270147 (+1.2%), Os 455460 -> 458620 (+0.7%);
  .text counts 16-B function alignment: f381 and f383 (`select c, 2, -2`
  i64) grow 29 -> 33 B and cross a slot (+16 each; O3: f381-f384, +64);
  the functions that shrink stay in their slot. Cause: base made one
  diamond for both halves in MIR (29 B); the DAG makes two CMOV pseudos
  and the second one's `mov -1` sits between them, so the cascade does not
  merge them (33 B). The pin's `f381` is 33 B too (clang -O2 -march=i486),
  so head is pin parity and base was a non-pin win (the MIR one-diamond
  expansion, which the pin does not make). External bars (m32 -march=i486,
  tools/external_flags.py cd1e12d510e4) for f381/f383: gcc 13.4 O1 32 B, O2
  43 B; clang O1 33 B, O2 33 B: head is 1 B over the best at O1, level at
  O2; base (29 B) beat both. Exit 2026-11-12: re-add the one-diamond
  expansion for the select of two i64 constants as a priced DEVIATE (or
  the pass the pin uses to keep `mov -1` out of the first CMOV window).
  The scheduler is right (traced: `MOV32r0` goes first, held by the live
  EFLAGS). `f409` is its own row, I64SEL-6.
- I64SEL-2, owner llrm-target-x86-m64 (SD447; oversight 2026-11-09: ties go
  to the pin), exit 2026-11-12: m64 `word_compares` -O1/-O2/-O3 295 -> 309 B,
  `main` 167 -> 173 B (pin 175 B); native instructions 139969 -> 140364
  (O1, +0.28%), 139968 -> 140365 (O2), 139969 -> 140364 (O3); cycles within
  noise, output equal; m32 `word_compares` identical. Cause: SD447 keeps
  `x*257` as `imul` on a tie, so the ADC takes `2*setbe` as an `add`
  before it, the flags are clobbered, the `cmp` is repeated with its load
  unfolded and one more value is live (R15D). Without the multiply the pin
  emits our shape. Evidence: the m64-mul257-adc-flags handoff note (2026-11-09).
- I64SEL-3, exit 2026-11-10 18:00Z: m64 `sfm16_float*` -O1 +8 B, -Os +10 B
  (only `__divsf3`: 493 -> 503 B, pin 560 B; register naming and one
  `mov`). Native instructions equal within 1 at O1 and Os (sfm16_float_add
  O1 128138 -> 128139, Os 128548 -> 128548), output equal.
- I64SEL-4, exit 2026-11-10 18:00Z: m32 `x_sf_cmp64` -Os +1302 executed
  instructions (+1.1% clocks) at equal bytes (`test; js` became
  `test; jns`, a branch layout flip).
- I64SEL-5, owner z80, exit 2026-11-12: z80-sdcc `call_clobbers_flags`
  -O2/-Os .text 602 -> 618 B. Cause: the shared `add (sext i1 c), x` ->
  `sub x, (zext c)` fold (pin `visitADDLikeCommutative`) now fires; `add
  hl,de` (11 T) becomes `ex de,hl; or a,a; sbc hl,bc` (23 T), +12 T per
  execution of the site (static estimate, not measured; ucsim run PASS,
  the runner prints no T).
- I64SEL-6, owner w12 (w9 traces), exit 2026-11-12: `f409` (`x<y ? INT64_MIN/2^31
  : 0`, m32 -march=i486, symbol size): base 25 B (O1, O2), head 30 / 32 B,
  clang 28 / 28, gcc 32 / 27: over the best external by +2 (O1) and +5
  (O2). Head emits `setl al; and eax,255; shl eax,31`: the CMOV diamond
  comes before the SETCC, FixupSetCC does not fire, MaskByAnd masks. Card:
  the f409-cmov-before-setcc handoff note (2026-11-09). Its growth is part
  of I64SEL-1's .text total.

The loop is repeated until no select is left in any function, so two selects
in one block each get their own split. The result is checked by an
interpreter test that runs a function before and after.

## Example
Before:

```mir
define i16 @f(i16 %a, i16 %b) {
entry:
  %less = icmp ult i16 %a, %b
  %least = select i1 %less, i16 %a, i16 %b
  %twice = add i16 %least, %least
  br label %done
done:
  %r = phi i16 [ %twice, %entry ]
  ret i16 %r
}
```

After:

```mir
define i16 @f(i16 %a, i16 %b) {
entry:
  %less = icmp ult i16 %a, %b
  br i1 %less, label %0, label %1
0:
  br label %1
1:
  %2 = phi i16 [ %a, %0 ], [ %b, %entry ]
  %twice = add i16 %2, %2
  br label %done
done:
  %r = phi i16 [ %twice, %1 ]
  ret i16 %r
}
```

The select became a branch to the empty block `0` and a phi in `1`; the phi
of `done` now names `1` as its predecessor.

From unit test `test_a_select_becomes_a_branch_and_a_phi` in `selects.rs`.

## Runs
Once per program in `driver::optimized`, after `fpconvert` and before the
spill-model dump, only where the target's `datalayout.toml` lists `select`
under `expand` (m16 and m32: no conditional move before the Pentium Pro).
There is no flag; it runs at every -O level.

## See also
LLVM's CodeGenPrepare (`optimizeSelectInst`) makes the same change where the
target prices a branch below a conditional move.

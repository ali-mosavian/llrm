> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/pushfold.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/pushfold.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/pushfold.rs sha256:9f3b5a7c6cfce923e57849417076a292d553b2d52a2cd3dadc0401af81367493 -->

# pushfold

A value that a load made and nothing else reads is read from memory where it
is pushed or added: `mov r, [m]; push r` is `push [m]`, and `v2 := add v0,
v1` with `v1` such a load is `v2 := add v0, [m]`, whichever operand of a
commutative operation the load is. Translated from
[X86CallFrameOptimization.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp)
(`canFoldIntoRegPush`, and the `PUSH32rmm` branch of `adjustCallSequence`).

## How it works
A `push` whose one source is a value qualifies when:

- the value is read once in the body (`hasOneNonDBGUse`);
- its definition is the nearest earlier one in the block, a plain `mov` of a
  memory cell of the value's width (a 2- or 4-byte cell: `push word ptr`,
  `push dword ptr`), not volatile, with no clobbers, requirements or group,
  and not a spill reload;
- nothing between the load and the push is volatile, a call, a barrier or an
  escape, or may write the cell (`isLoadFoldBarrier`: the pin's check is the
  same walk, from the load to the call frame setup);
- the cell is not addressed through the stack pointer, whose value the
  earlier pushes of the sequence move.

The push then reads the cell and the load is deleted. The pin does this when
it turns the stores of a reserved call frame into pushes; ours pushes at
selection already, so the fold is all that is left of the pass (DEVIATE
CF1). The pin skips it where `slowTwoMemOps` holds; the i486 and the 8086 do
not set it.

DEVIATE CF1 (no number): the pin's input is `mov [esp+k], r` stores into a
reserved frame, converted to pushes and folded in one pass; ours has pushes
from selection, so the conversion has no input and `isProfitable` (its
reserved-frame accounting) no question. Pin llc -m32 i486, -Os and -Oz:
`movl %edi,(%esp); calll g`, no push, no cleanup per call. Exit: none, the
form is not made (debt row call-frame-reserve, docs/working/ci.md).

## Example
`f(p->a, p->b, p->c)`, before (after `FloatFold`):

```lir
g: 9 instructions
  block 0x0000
    0x0001  v2 := mov [v1 + 8]
    0x0002  push v2
    0x0003  v3 := mov [v1 + 4]
    0x0004  push v3
    0x0005  v4 := mov [v1]
    0x0006  push v4
```

After:

```lir
g: 6 instructions
  block 0x0000
    0x0002  push [v1 + 8]
    0x0004  push [v1 + 4]
    0x0006  push [v1]
```

## Runs
Twice. As `LoadFold` first of the machine phases, after selection: the pin's
DAG selection folds a once-read load into `ADD32rm` before MachineCombiner
runs, and a load folded after it let the combiner rebuild a chain as a tree.
As `PushFold`, a machine phase after `floatfold`, before the register
classes are decided,
once per function, at every -O level but -O0 (the pin's pass is
`-no-x86-call-frame-opt`-switchable only, and runs at -O0 too; ours does not
at -O0, where nothing improves after selection).

## Needs
[`crate::backend::storedhomes`](storedhomes.md) for the may-write test.

## See also
[`crate::backend::floatfold`](floatfold.md), the same fold for x87 arithmetic.

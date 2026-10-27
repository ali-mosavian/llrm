> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_select_into_op.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_select_into_op.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_select_into_op.rs sha256:0897bc5a41724b3a725be59763fc5959a721f4d64f93dec89f3bd86eab7ca584 -->

# combine_select_into_op

`foldSelectIntoOp`: `select c, (op x, y), x` as `op x, (select c, y,
identity)`.

Translation of
[`InstCombinerImpl::foldSelectIntoOp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L559)
at d1106deb71cc, with `getSelectFoldableOperands` (:251): the select picks
only the operand that differs, and the operation runs on either path. The
shape `if (x >= r + bit) { x -= r + bit; ... }` makes
`select c, (sub x, y), x`; the operation then runs after a select of `y` or
zero, not before a select of its result.

MIRROR: the operation has one use; the other arm is no constant; it is
the operation's first operand (any of them for a commutative operation); the
operation's other operand, if a constant, must make a select of 0, 1 or -1
with the identity; the new operation keeps the old one's flags.

DEVIATE 1: only integer operations. The floating ones (`fadd`, `fsub`,
`fmul`, `fdiv`) need the select's fast-math flags and `isKnownNeverNaN` of
the other arm, which `computeKnownFPClass` answers where #2209 lands.

DEVIATE 2: the new operation is not named after the old one (`takeName`):
MIR has no rename.

## Example
```text
%d = sub i32 %x, %y                      %s = select i1 %c, i32 %y, i32 0
%r = select i1 %c, i32 %d, i32 %x   ==>  %r = sub i32 %x, %s
```

## Used by
Named by `combine_visit_select`.

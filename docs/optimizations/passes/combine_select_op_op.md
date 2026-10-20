> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_select_op_op.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_select_op_op.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_select_op_op.rs sha256:6d0ad05160e29f9c6a007a0726513d51a4a6387ab6acfca64990a30f6256d9e7 -->

# combine_select_op_op

`foldSelectOpOp`: `select c, (op x, a), (op x, b)` as `op x, (select c, a,
b)`.

Translation of
[`InstCombinerImpl::foldSelectOpOp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L274)
at d1106deb71cc, for arms of one opcode: two casts of one source type, two
`fneg`s, an `icmp` pair with a common operand, a binary operator or
two-operand GEP pair with one.

MIRROR: the one-use conditions, which operand is common (a commutative
operation may match across), the `freeze` of a condition that could be
poison before a division moves under it, the flags both arms had.

DEVIATE 1: no vectors. Min/max intrinsics with a common operand, and
`ldexp`, are not made: the Builder cannot declare an intrinsic
(`combine_build` DEVIATE 2), and a call is cloned only with its own callee.

DEVIATE 2: the two `icmp`s match when their predicates are equal (or equal
once one is swapped): `CmpPredicate::getMatching`'s `samesign` cases are not
found (MIR's `icmp` has none).

## Example
```text
%x1 = add i16 %x, %a                  %v = select i1 %c, i16 %a, i16 %b
%x2 = add i16 %x, %b            ==>   %r = add i16 %x, %v
%r = select i1 %c, i16 %x1, i16 %x2
```

> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/divrempairs.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/divrempairs.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/divrempairs.rs sha256:933cd50d4d885c17c3299928635d2bbe38a50501f71517915409a76d6f81c007 -->

# divrempairs

A division and the remainder of the same operands are brought together, or
one made of the other, as LLVM's `DivRemPairs`.

## How it works
A translation of LLVM
[`DivRemPairs.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/DivRemPairs.cpp)
at d1106deb71cc. A `sdiv` or `udiv` and a `srem`, `urem` or expanded
remainder `x - (x / y) * y` with the same dividend, divisor and signedness
are a pair. Where the target has a divide that makes both
(`TargetTransformInfo::hasDivRemOp`: x86 on every register type, or what a
target's `Tti` says), the expanded remainder is first written as the real
`srem`/`urem`, and the pair is put side by side so the backend finds one
divide: the one in a block the other's block follows is moved after it, and
a pair in two arms of one branch goes to the block before the branch when
nothing before either can fail to reach it. Where the target has none, a
`srem`/`urem` becomes `x - (x / y) * y` of the quotient already made, and
the quotient moves up to the remainder when it stood below it.

## Example
```text
 before                          after (no divide-remainder instruction)
 %d = sdiv i32 %x, %y            %d = sdiv i32 %x, %y
 ...                             ...
 %r = srem i32 %x, %y            %m = mul i32 %d, %y
                                 %r = sub i32 %x, %m
```

An operand that may be `undef` is frozen first, one value for the
quotient and the remainder, and the quotient loses its `exact` flag.

DEVIATE 1: `CatchSwitchInst` terminators have no MIR spelling, so the check
that refuses to hoist into one has nothing to refuse.

DEVIATE 2: `hasDivRemOp` is read at two sites, `Tti::has_div_rem_op`
(default false) and `Lowering::has_div_rem_op`, and either one true counts.
The pin has one, `TTI.hasDivRemOp(Type, IsSigned)`
([DivRemPairs.cpp#L194](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DivRemPairs.cpp#L194)),
which `X86TTIImpl` answers
([X86TargetTransformInfo.cpp#L7421](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L7421)).
Reason: x86 states its answer on its `Lowering`; the 6502 states its on its
`Tti`. With the default false, x86 and arm64 are as they were. Exit: x86
states it on its `Tti`, then the `Lowering` site goes. Owner: the 6502
goal, in its own small PR with an x86 m16, m32 and m64 sweep and a control.

## Parity slot
`divrempairs@1667`, in the late function pipeline between `instsimplify`
and `tailrec` (PassBuilderPipelines.cpp#L1667).

## Keeps
Blocks and edges are unchanged: dominators and loops are preserved.

## Used by
Named by `pipeline`.

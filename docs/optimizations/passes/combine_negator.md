> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_negator.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_negator.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_negator.rs sha256:bebc4ce80be6b05e077299e95ad176e82615eb050d5f9576c609069cbccc70b6 -->

# combine_negator

`Negator`: sinks a negation into an expression tree when that does not add
an instruction.

Translation of
[`InstCombineNegator.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineNegator.cpp)
(`Negator::Negate` :547, `run` :526, `negate` :485, `visitImpl` :114).

MIRROR: the C++ order of cases and the cache of negations tried. The
Builder is `TargetFolder`'s: an operation on constants folds, any other is
inserted before the instruction being negated. A failed negation erases
what it created, last first.

DEVIATE 1: no vectors: the `shufflevector`, `extractelement` and
`insertelement` cases do not exist in MIR.

DEVIATE 2: the `scmp`/`ucmp` case (`CmpIntrinsic`, :201) is not
translated: it builds an intrinsic call (`combine_build` DEVIATE 2).

DEVIATE 3: no statistics, `DebugCounter` or `-instcombine-negator-enabled`
(always on). `NegatorDefaultMaxDepth` is `~0U` at the pin: the recursion
is as deep as the tree of one-use operands, as in LLVM.

DEVIATE 4: constant expressions are not negated (MIR keeps them for
global initializers only).

## How it works
`negate` asks the cache, else `visit`; `visit` answers the cases that need
no recursion first, then those that need one use, then recurses into the
operands. New instructions are queued on the worklist at insert, in the
order `Negate` hands them to `addToWorklist`.

## Used by
`combine_addsub::visit_sub`, `combine_muldivrem`.

## Example
`0 - (x + 1)` is `~x`; `0 - (a - b)` is `b - a`.

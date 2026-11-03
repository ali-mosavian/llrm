> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/lsr_solve.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/lsr_solve.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/lsr_solve.rs sha256:4feb3565dadea22537f478e2fad7536d53451916f02f9c4717b2747a0e5a7cdb -->

# lsr_solve

The solver of LLVM's loop strength reduction: one formula per use, by a
depth-first search pruned by the best cost found (`SolveRecurse` and
`Solve`; LoopStrengthReduce.cpp at `d1106deb71cc`, l.5573-5710).

## How it works
Uses are decided in order. A formula of a use must reference every register
already chosen that the use could also reference (up to its register count),
which prunes most of the search; its cost is rated on top of the cost so
far, and the branch is dropped when it is no better than the best solution.
A formula of the first use that is a single register is remembered as
visited: formulae later in the search that name it lose.

## Example
The array walk and its exit test: the store keeps `{%p,+,4}`, the compare
keeps `{(-1 + %n),+,-1}`, one register each, as `opt -passes=loop-reduce`
has them.

## Runs
Nothing runs it yet: stage 5 of the translation of LSR.

## Limits
DEVIATE: `AMK_PreIndexed` is not ported, and so a loop that wants `-(An)`
(a pointer stepped down before each access) stays a step and an access.
The pin sites: `RateRegister`'s `CanPreIndex`
([L1468](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L1468))
and the offset-minus-step formulae of `GenerateConstantOffsetsImpl`
([L4230](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L4230)).
No target of ours prefers it (`getPreferredAddressingMode` is `PostIndexed`
or `None`), and the overlay has no `-(An)` fold either. Exit: with the first
target that prefers pre-indexed addressing. The
`-lsr-preferred-addressing-mode` option (L170) is not either: the target's
hook decides. Ported, for `AMK_PostIndexed`: L1445, L1461-1471, L3780,
L3988, L4735, L5196, L5609 and `getFixupInsertPos` (L6190,
`lsr_implement.rs`; a store keeps the default position, as the pin's
`I->getType()` is `void` for it). DEVIATE LU3: see `solve`, the drop of a
rewrite that spills (real mode).

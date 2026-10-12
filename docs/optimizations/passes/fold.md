> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/fold.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/fold.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/fold.rs sha256:e209148e85709a3731d9195063efbed742e24c3eb50dde659503d7822ab3eb9e -->

# fold

Replaces every integer value [`llrm_analysis::consts`](../analyses/consts.md) knows by its
constant, folds a pure operation on each edge of a join, and puts compares
and identities in canonical form.

## How it works
Facts come from `consts`: "the low `width` bits of this value are `n`",
solved through registers, phis and memory (a load of a cell every reaching
store fills with one number is that number). What each call writes is
`manager::writes`, so a constant stored before a `readonly` call survives
it. A fact replaces its value only when it is as wide as the value. A
division or remainder of two known numbers is computed here too.

Each use of a known value becomes the constant at once, so a store's value,
a call argument, a fill's byte or an address index need no case of their
own, and the definition is left unread for [`crate::dead`](dead.md) to take. A
commutative operation that ends up with the constant on the left gets it
back on the right; an ordered one (`sub`, shifts) keeps its operand order.

A call kills only the cells its writes may overlap (`CallEffects`, from
`Summaries` and the call's `memory(...)`, `readonly` and `readnone`
attributes); one that states writes without an effect list clears every
cell. A fact about reaching an edge says nothing about a callee, so edge
facts clear all cells at a call.

```text
 store i16 9, ptr @g          store i16 9, ptr @g
 %a = load i16, ptr @g   ==>  call void @clobber()   ; writes @g
 call void @clobber()         %b = load i16, ptr @g  ; unknown again
 %b = load i16, ptr @g        (%a is 9; with @clobber readonly, %b too)
```

One edge fold runs per call. At a block with two or more predecessors, a
pure `Binary` or `Cast` in the block (or in the single-predecessor chain
below it) that reads one of its phis, and whose answer is a number on every
incoming edge, becomes a phi of those numbers. An operation that also reads
an unknown value stays.

```text
 b3:                                 b3:
   %p = phi [1,%b1], [2,%b2]           %p  = phi [1,%b1], [2,%b2]
   %y = add %p, %x                     %s1 = phi [11,%b1], [12,%b2]
   %s = add %p, 10          ==>        %y  = add %p, %x
   %t = add %s, %y                     %t  = add %s1, %y
```

Then [`crate::floatfold`](../backend/floatfold.md) runs on the same body (exact floats, and the
integers of exact conversions), and [`crate::canonical`](canonical.md) swaps a compare
whose constant is on the left and simplifies identities. The pass reports
every analysis invalid when anything changed.

## Runs
Second in the per-body pipeline, after `sroa`, and again after the header
copy of `rotate` (to clean the test it leaves in the latch). Part of the
fixed point; on at -O1 and up, off at -O0. `Options::fold` turns it off
for bisection. See `docs/reference/levels.md`.

## Needs
[`llrm_analysis::consts`](../analyses/consts.md), [`llrm_analysis::floatfacts`](../analyses/floatfacts.md),
`manager::writes` (`Summaries`).

## See also
LLVM's
[`SCCP`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp)
and `InstSimplify`; GCC's `tree-ssa-ccp.cc`.

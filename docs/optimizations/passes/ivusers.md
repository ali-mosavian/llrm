> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/ivusers.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/ivusers.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/ivusers.rs sha256:ac8b8f532fc2112fca4d2402f23dbec3ad77a5034a6227ea07752f572aa049bb -->

# ivusers

The users of a loop's induction variables (`IVUsers.cpp` at `d1106deb71cc`),
which loop strength reduction rewrites.

## How it works
From each phi of the loop header, follow the users while the SCEV of the
value stays "interesting" (an affine add recurrence of the loop, or an add
with exactly one such operand). A user that is not itself reducible is an
`IvStrideUse`. For a user outside the loop that the latch dominates, the
post-incremented value is used: the expression is normalized for the loop,
and the use is dropped if denormalizing does not give the original back.

## Example
`a[i]` in `for (i = 0; i < n; i++)`: the load's address is `{a,+,4}<loop>`,
one use whose user is the load and whose operand is the address.

## Runs
Nothing runs it yet: stage 3 of the translation of LSR.

## Limits
DEVIATE IU1: users of a value are visited in descending program order, not
use-list order. Use-list order is edit history; the pin's order on parsed
IR is the only reproducible one, and it breaks ties in LSR's register
choice. Cost (IU1-COST): one map of the layout per loop and a block's
instruction numbers, where the pin has none (it was a map of every
instruction: 1.9 M on d_faces -O2). Exit: use lists keep LLVM's order.
Owner: w-5. Review: 2026-11-19.

DEVIATE: whether an instruction is safe to speculate and whether it is
ephemeral are asked of the caller (`Env`), which holds what they need.
Walks each value's users once, with a visited set: O(users).

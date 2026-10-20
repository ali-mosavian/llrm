> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/ivusers.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/ivusers.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/ivusers.rs sha256:07d050f7087f119b9b4a3f131eeb076c4c201c8d37bcd6fb4fdcade6a9583a7d -->

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
DEVIATE: whether an instruction is safe to speculate and whether it is
ephemeral are asked of the caller (`Env`), which holds what they need.
Walks each value's users once, with a visited set: O(users).

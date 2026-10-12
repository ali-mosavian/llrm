> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/exitfold.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/exitfold.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/exitfold.rs sha256:32c1fb7a1d9f2f8dea27d4d3414e07992cbc86219eba75d192eb8b773d868971 -->

# exitfold

Exits their counts decide, folded: LLVM IndVarSimplify's
`optimizeLoopExits`. Of a loop's exits the latch follows, each counted
(`induction::exits`), in dominance order:

- one whose count an earlier exit shares never leaves: the earlier one
  leaves on that trip first;
- one whose count is above the loop's most, the least of the others', as the
  constants or the guards on entry prove, never leaves;
- one whose count is zero leaves on the first trip.

Its branch is given the condition that decides it; `decide` makes it a
jump. Nothing else changes, so no effect or value between the exits
matters.

An exit no count decides, whose compare tests a counter stepping by one
against an invariant, is tested once instead, before the loop, where
the guards prove it holds on every trip up to the loop's most once it
holds on the first: LLVM's `optimizeLoopExitWithUnknownExitCount`. The
test is of the counter's start; failing it, the loop leaves on the first
trip, as it did. That test is itself a fact the proof may use, with the
ranges the program states (`guards::holds_given`).

Exits that leave to the same place with the same values, nothing seen
or trapping between them, become one: the first leaves on the trip the
least of all their counts ends, testing its counter against its value
then, and the others never leave. Whichever would have left first, the
first leaves on that trip, earlier in it, to the same place: nothing in
between could be seen. Neither LLVM nor GCC merges live exits; this is
what lets a loop bounded by several lengths count one.

A loop that writes nothing, or only plain stores where each exit it
leaves by early crashes at once touching no memory the program sees,
and whose exits carry no values out, leaves on its first trip through
the exit it would have left by: each exit, in dominance order, leaves
where its count is the loop's. LLVM's `predicateLoopExits`, with its
traps.

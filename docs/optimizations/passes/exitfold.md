> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/exitfold.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/exitfold.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/exitfold.rs sha256:add09964d9b76944317157644c31c3354f0b25dee0089a383d33d067efc5ab9a -->

# exitfold

Exits their counts decide, folded: LLVM IndVarSimplify's
`optimizeLoopExits`, with its exit-test hoisting and exit predication.

## How it works
For each loop, `induction::exits` gives every exit the latch follows, each
with the trip count that makes it leave. The exits are taken in dominance
order. Deciding and testing once are tried on every loop; merging, and then
predication, only for a loop where neither found anything. Each reads the
exits as they were.

Decided exits. An exit whose branch is not on a constant, in the loop itself
and not an inner one, is decided when

- its count is zero: it leaves on the first trip;
- an earlier exit has the same count: the earlier one leaves on that trip
  first, so this one never leaves;
- some other exit's count is below all of its own, by the constants or by
  the guards on entry (`guards::holds`): it never leaves.

The exits and their counts are ScalarEvolution's (`getExitCount`,
`isLoopEntryGuardedByCond`: LLVM's `optimizeLoopExits`: zero, below the
loop's symbolic maximum, a count an earlier exit has). `LLRM_NO_SCEV_EXITFOLD`
restores `induction::exits`; the two differ on 2 of 1,590 corpus objects,
both smaller.

The branch is given the constant that decides it; `decide` then makes it a
jump. Nothing else changes, so no effect or value between the exits matters.

```text
 header:  br i1 (i < n),   body, exit1     header:  br i1 (i < n), ...
 body:    br i1 (i < len), next, exit2 ==> body:    br i1 true, next, exit2
 guard on entry: n < len                   (i < len holds on every trip)
```

Tested once. An exit no count decides, whose compare tests a counter
stepping by one against an invariant, is tested once, before the loop
(LLVM's `optimizeLoopExitWithUnknownExitCount`). The guards prove that, once
the test holds on the first trip, it holds on every trip up to the loop's
most. The test is of the counter's start; failing it, the loop leaves on the
first trip, as it did. The test itself is a fact the proof may use, with the
ranges the program states (`guards::holds_given`). Where the first trip
reaches the exit with nothing seen or trapping on the way, the preheader
branches out directly; otherwise the exit's branch takes the hoisted
condition.

Merged exits. Exits that leave to the same place with the same values (an
LCSSA block of phis only its jump reads is passed through), nothing seen or
trapping between them, and counters of one width, become one: the first
leaves on the trip the least of all their counts ends, testing its counter
against `start + step * least(counts)` then, and the others never leave.
Whichever would have left first, the first leaves on that trip, earlier in
it, to the same place: nothing in between could be seen. Neither LLVM nor
GCC merges live exits; this lets a loop bounded by several lengths count
one. The first exit must be tested as the trip begins (not stepped, not
post-tested, not `!=`).

Predicated exits. A loop that writes nothing, or only plain stores where
each exit it leaves by crashes at once touching no memory the program sees,
and whose exits carry no values out, leaves on its first trip through the
exit it would have left by: each exit, in dominance order, leaves where its
count is the loop's, tested in the preheader. LLVM's `predicateLoopExits`,
with its traps.

Values are built before the loop by `expand`'s `Expander`.

## Example
Before, `entry` proves `n < len`:

```mir
head:
  %i = phi i16 [ 0, %entry ], [ %j, %body ]
  %more = icmp ult i16 %i, %n
  br i1 %more, label %check, label %done

check:
  %inside = icmp ult i16 %i, %len
  br i1 %inside, label %body, label %done
; ...
```

After:

```mir
check:
  %inside = icmp ult i16 %i, %len
  br i1 true, label %body, label %done
```

The header leaves at `i == n`, before `i < len` can fail, so the check's
exit never leaves and its branch is the constant `true`; `decide` then makes
it a jump.

From unit test `test_an_exit_the_guard_proves_later_never_leaves` in
`exitfold_tests.rs`.

## Runs
Step 2 of [`crate::indvars`](indvars.md), so on whenever `indvars` is, in the fixed
point and after the loop-header copy: not at -O0, and off with
`-fno-strength-reduce`.

## Needs
`induction::exits`, `guards`, `consts`/`ranges` through `Standing`, and the
CFG shape ([`crate::expand`](expand.md), [`crate::counting`](counting.md) build the values).

## Keeps
Blocks and edges change (preheader tests, new blocks): every analysis is
invalidated when anything folded.

## See also
[`crate::decide`](decide.md), [`crate::exitsink`](exitsink.md); LLVM IndVarSimplify
(`optimizeLoopExits`, `predicateLoopExits`).

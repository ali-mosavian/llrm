> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopexit.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopexit.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopexit.rs sha256:71dce46f1fd6fb4b66281f28df93701e999dfed285dee9361c39518fc67ccfb4 -->

# loopexit

Replaces what a counted loop computes, as read after it, by an expression of
its trip count, and deletes a loop that only counts, so the code after the
loop does not wait for the loop to produce a number the compiler can write
down. LLVM's IndVarSimplify exit-value rewriting and LoopDeletion for loops
that compute only what is read after them.

## How it works
A loop is considered when it has two blocks (a header and one latch with no
phis), at least one counter (`induction::basics`), a counted exit whose
trip count `N` is agreed by every proof, the test before the step (not
post-tested) and a preheader. Only the controlling recurrence must be
proven not to wrap; the rest is arithmetic modulo each value's own width.

For each header phi the pass looks for its value on leaving, a sum of terms
(a value or number times a coefficient), the integer equivalent of
evaluating an add recurrence at its backedge count:
- a counter is `start + N * step`;
- a phi whose update is linear in the header's values, the loop's
  invariants, other counters and recurrences adds, per trip, a fixed
  increment `c` as `N * c`; an increment that is itself a counter `s + t *
  i` also adds `N(N-1)/2 * t`;
- a counter that a `sext` widens, where `ranges` bound it at its own width,
  is read as the wide recurrence it is;
- a phi with a product of unknowns, or an update that is not linear, has no
  exit value.

With the exit values, one of two things happens.
- Every header phi has one and the loop is its counting alone
  (`_disposable`: only phis, `add`, `sub`, `icmp` and `br`, none read
  outside the loop but the header's phis): the header computes the exit
  values (`counting::Seeds`), replaces the phis, and branches to the exit;
  the latch becomes unreachable.
- Otherwise, where the exit has the header as its only predecessor, a value
  read after the loop whose terms are all known numbers is replaced there by
  its constant (`Evaluation::Constant`); the loop stays, and what it still
  computes for itself is `lsr`'s.

The pass repeats one loop at a time until nothing changes, deriving what is
known of the body again after each.

## Example
`s = 0; for (i = 0; i < 4; i++) s += i; return s`. Before:

```mir
@g = global i16 0

define i16 @f() {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %s = phi i16 [ 0, %b0 ], [ %s1, %b2 ]
  %c = icmp uge i16 %i, 4
  br i1 %c, label %b3, label %b2

b2:
  %s1 = add i16 %s, %i
  %next = add i16 %i, 1
  br label %b1

b3:
  ret i16 %s
}
```

After:

```mir
define i16 @f() {
b0:
  br label %b1

b1:
  br label %b3

b3:
  ret i16 6
}
```

Every header phi has an exit value (`0 + 1 + 2 + 3` is 6), so the header
branches to the exit and the latch is gone.

From unit test `disposable_loop_becomes_its_exit_values` in
`loopexit_tests.rs`.

## Runs
Not a pipeline pass of its own: [`crate::indvars`](indvars.md) calls
`evaluated_with` first in each run, so it runs wherever `indvars` does
(every level above -O0 with `-fstrength-reduce`). `LoopExit` is the same
step as a function pass, for tests.

## Needs
`Registers`, and `Bounded` where a `sext` of a counter is present;
[`crate::counting`](counting.md) to build values.

## Keeps
Nothing where it changes a body.

## See also
llrm-mir's `loopdeletion` deletes a loop of any shape whose exit values
come from outside it, and evaluates none: a loop computing what is read
after it stays there. LLVM's `rewriteLoopExitValues` and LoopDeletion.

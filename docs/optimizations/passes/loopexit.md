> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopexit.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopexit.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopexit.rs sha256:e792e860f12a3cf7048cb667062cd05baba56dc3db6239a6f0605d20ae474ae6 -->

# loopexit

Affine exit values evaluated, and a counted loop that only counts deleted:
LLVM's IndVarSimplify exit-value rewriting and LoopDeletion for loops that
compute only what is read after them.

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

```text
  s = 0; for (i = 0; i < 4; i++) s += i;  return s;

  h: i = phi [0,pre] [i+1,l]            h: br exit            ; 0+1+2+3
     s = phi [0,pre] [s+i,l]     =>     exit: ret 6
     br i < 4, l, exit
  l: ...  br h
```

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

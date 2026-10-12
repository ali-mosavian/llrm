> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/trivialunswitch.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/trivialunswitch.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/trivialunswitch.rs sha256:a90a653ce2c829c20360f3214ad98c605a1ec35f952046d0db7e187d7ba9d568 -->

# trivialunswitch

A branch in a loop's header on a loop-invariant condition, one side of
which leaves the loop, is decided in the preheader instead of on every
trip.

## How it works
The loop needs one preheader that jumps only to the header. The header's
terminator is a conditional `br` on a value defined outside the loop, with
one target outside the loop (the exit) and one inside. Whichever way the
branch goes on the first trip it goes on every trip, so the preheader takes
that branch and the header falls through to the loop body.

Nothing is copied. Three conditions keep the move sound:
- What the header computes before its branch is phis and work that can be
  speculated, because it is skipped where the loop is never entered.
- A value the loop defines is used outside it only by the exit's phis.
- The exit's phis take their value on the new edge from a value defined
  outside the loop, or from a header phi's value on entry. Any other value
  the loop computes is not there yet, and keeps the branch.

```text
  before                         after
  pre:  br head                  pre:  br %z, out, head
  head: %i = phi [..]            head: %i = phi [..]
        br %z, out, test               br test
  test: ...                      test: ...
  out:  phi [%acc, head], ..     out:  phi [%acc_entry, pre], ..
```

Tail recursion makes such loops: `paths(n, m)` tests `n == 0` on every trip
of the loop that tests `m == 0`. A condition inside the body, or a branch
that stays in the loop on both sides, is [`crate::unswitch`](unswitch.md)'s, which
copies the loop.

The pass repeats while a loop changes.

## Runs
In the function pipeline's fixed point, after `loopmotion` and before
`gvn`. At every level above -O0; there is no flag for it alone.

## Needs
The loop shape ([`crate::loopsimplify`](loopsimplify.md) gives the preheader) and callees'
effects, to tell work that can be speculated.

## Keeps
Nothing: a change invalidates every analysis.

## See also
LLVM's SimpleLoopUnswitch, trivial form (`unswitchTrivialBranch`). It
follows a chain of blocks that each continue unconditionally; this pass
looks only at the header.

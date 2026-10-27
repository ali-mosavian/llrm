> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/lcssa.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/lcssa.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/lcssa.rs sha256:8a4f4bafb3a3cc4acbecb4634228bd75c4a9d7ab86ae28858149be858a444467 -->

# lcssa

Puts every natural loop in loop-closed SSA form, so loop passes can change a
loop without rewriting the readers of its values elsewhere.

## How it works
A value defined in a loop and read outside it is read through a phi in the
loop's exit block. Later loop passes can then change the loop's body, copy
it or move its exits without rewriting readers outside it.

Loops are taken innermost first, so an inner exit phi is an ordinary
definition of the enclosing loop. For one loop:

- With a single exit block whose predecessors are exactly the exiting
  blocks, a value is closed when every block that reads it is dominated by
  the exit and its definition dominates every exiting block. Each such value
  gets a phi in the exit with one input per exiting block, and every outside
  reader the exit dominates is rewritten to read it. A phi that reads the
  value on an edge counts at that edge's source block. If the exit has other
  predecessors, the loop is left for `loopsimplify`.
- With several exit blocks, [`crate::lcssamerges`](lcssamerges.md) makes a phi at each and
  merges them where the exits' paths join.

Phis are only added, as `formLCSSA` does: a phi whose inputs are all one
value stays for CodeGenPrepare's phi fold. The new phi is named after its
value with `.lcssa`. The pass is idempotent.

The helpers `arms`, `from_arms`, `operations`, `definitions`, `exit_phi` and
`place_phi` read and build phis for many other loop passes.

## Example
Before:

```mir
define i16 @f(i16 %seed, i16 %n) {
b0:
  br label %b1

b1:
  %carried = phi i16 [ %seed, %b0 ], [ %stepped, %b2 ]
  %more = icmp ult i16 %carried, %n
  br i1 %more, label %b2, label %b3

b2:
  %stepped = add i16 %carried, 1
  br label %b1

b3:
  %answer = add i16 %carried, 7
  ret i16 %answer
}
```

After:

```mir
b3:
  %carried.lcssa = phi i16 [ %carried, %b1 ]
  %answer = add i16 %carried.lcssa, 7
  ret i16 %answer
; ... b0 to b2 unchanged
```

`%carried` is read in `b3`, outside the loop, so the exit block gets a phi
of it and the reader reads the phi.

From unit test `test_a_loop_value_used_after_the_exit_gets_an_exit_phi` in
`lcssa_tests.rs`.

## Runs
In every body's pipeline after `loopsimplify`, unless `lcssa` is turned off,
and again after header copying. The unroller calls it first.

## Keeps
Dominators and loops stay valid; all other analyses are invalidated when
something changed.

## See also
LLVM's LCSSA (LCSSA.cpp, `formLCSSA`).

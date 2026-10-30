> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/counting.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/counting.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/counting.rs sha256:52d38d3864ea7b55cb318634ef8425162f72c97e661ca6ee4ede8b6ad147f1be -->

# counting

What a counted-loop rewrite places before the loop: its seeds, skip test and
exit values (LLVM's SCEVExpander, as IndVarSimplify and LoopRotate use it).

## How it works
Everything is built from an `induction::CountedLoop` proof, so every pass
that replaces a loop's counter enters and leaves the loop the same way. A
`Seeds` names where the arithmetic goes (before `at`) and how wide it is.

- `Seeds::computed` places `kind` of its arguments, or the constant where
  both are constants, so a rewrite whose count and start are known places
  nothing; `canonical::identities` folds the neutral terms of the rest.
- `Seeds::widened` zero-extends a trip count to an index's width.
- `Seeds::operand` and `constant` turn an affine term or a number into an
  operand of a given width.
- `skip_guard` builds the preheader compare that is true when the loop runs
  no trips; the caller branches on it to the loop's exit.
- `leaving` gives the new operands of the exit phis of a replaced counter:
  its exit value after a trip and, where a guard may skip the loop, its
  start after none.

```text
 preheader:                         built before `at`:
   br loop                  ==>       %trips = ...        Seeds::computed
                                      %skip  = icmp ...   skip_guard
                                      br %skip, exit, loop
 exit:                              exit:
   %r = phi [%i, latch]               %r = phi [%v, latch], [%i0, preheader]
```

A seed is placed as it is made, so a caller asks for one only once it will
commit to its rewrite.

## Example
Before:
```mir
define i16 @f(i16 %s, i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ %s, %b0 ], [ %next, %b2 ]
  %c = icmp slt i16 %i, %n
  br i1 %c, label %b2, label %b3
; ...
b3:
  %e = phi i16 [ %i, %b1 ]
  ret i16 %e
}
```
After `skip_guard` and `leaving` (the lines the test asserts):
```mir
b0:
  %0 = icmp sle i16 %n, %s
  %1 = add i16 %n, 0
  br i1 %0, label %b3, label %b1
; ...
b3:
  %e = phi i16 [ %1, %b1 ], [ %s, %b0 ]
```
The preheader tests for no trips and branches to the exit, and the exit phi
takes the exit value `%n` (from `add %n, 0`, left to
`canonical::identities`) or, with no trip, the start `%s`.

From unit test
`a_symbolic_exit_value_and_skip_test_are_placed_before_the_loop` in
`counting_tests.rs`.

## Used by
[`crate::loopidioms`](loopidioms.md), [`crate::window`](window.md) and
[`crate::lsr_pass`](lsr_pass.md) build `Seeds` when they commit to replacing a loop's
counter, and `lftr` and others take `constant` for operands.
Not a pass: it runs only inside those.

## See also
[`crate::expand`](expand.md) for sums.

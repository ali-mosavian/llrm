> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/counting.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/counting.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/counting.rs sha256:265890dd82583a0eb89c9a0a43a6c38d34fc63f8973f6c79ebc46864a63d4a94 -->

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

## Used by
[`crate::fill`](fill.md), [`crate::loopexit`](loopexit.md), [`crate::window`](window.md) and [`crate::lsr`](lsr.md)
build `Seeds` when they commit to replacing a loop's counter, and
[`crate::exitfold`](exitfold.md) and others take `constant` for operands. Not a pass: it
runs only inside those.

## See also
[`crate::expand`](expand.md) for sums.

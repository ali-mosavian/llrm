> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/alwaysundef.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/alwaysundef.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/alwaysundef.rs sha256:2e14013ec7ffe4a4d08894ba4d21251c03fce884ee12519c2e3bce36871213af -->

# alwaysundef

Whether passing a constant into a phi is undefined behavior at a use of
the phi: SimplifyCFG's `passingValueIsAlwaysUndefined`.

## How it works
A null (or undef) input of `phi` makes the program undefined where the
first use of `phi` after it in its block, with nothing in between that may
not come back, dereferences it, stores through it, calls it, divides by
it, returns it from a `noundef` function or passes it to a `noundef`
parameter; a use through a `getelementptr` from the pointer is looked
through. `removeUndefIntroducingPredecessor` and the folds that would make
a select of the phi ask it, to make the undefined control flow go instead.

## Example
A null the block loads through makes its way in undefined:

```text
  b1: br b3                       b1: unreachable
  b3: %p = phi ptr [ null, b1 ], [ %q, b2 ]   (b1's edge is gone)
      %v = load i16, ptr %p
```

## LLVM structure
| LLVM | here | |
|---|---|---|
| [`passingValueIsAlwaysUndefined`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9094) | `always_undefined` | MIRROR; D1, D2 |

- D1. `NullPointerIsDefined` is the function's `null_pointer_is_valid`
  attribute for every address space.
- D2. `isPassingUndefUB` is the `noundef` attribute of the call's argument;
  the callee's own parameter attributes are not read.

## Used by
Named by `hoistcommon`, `speculate`, `undefpred`.

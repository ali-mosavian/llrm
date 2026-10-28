> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/undefpred.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/undefpred.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/undefpred.rs sha256:6899096828822a8983a73eebff8873f591c0713266de065f9b501a05bc0edc6d -->

# undefpred

A predecessor that gives a block a value that is undefined behavior to use
there stops reaching it: SimplifyCFG's `removeUndefIntroducingPredecessor`.

## How it works
`removed` looks at each input of each phi of `block`. When
[`always_undefined`](alwaysundef.md) says the input is undefined where
the phi is used (a null the block loads through, say), the predecessor
that gives it ends differently: a jump becomes `unreachable`; a
conditional branch becomes a jump to its other target (or `unreachable`
where both targets are the block); a `switch` sends the cases that reach
the block to a new block that is `unreachable`. One predecessor per call.

## Example
The way in that gives the null is cut:

```text
  b1: br b3                       b1: unreachable
  b2: br b3                       b2: br b3
  b3: %p = phi ptr [ null, %b1 ], [ %q, %b2 ]      b3: %p = phi ptr [ %q, %b2 ]
      %v = load i16, ptr %p                            %v = load i16, ptr %p
```

## LLVM structure
| LLVM | here | |
|---|---|---|
| [`removeUndefIntroducingPredecessor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9230) | `removed` | MIRROR; D1 |

- D1. A conditional branch keeps no `llvm.assume` of its guarding condition:
  a function pass cannot declare `llvm.assume`, a global of the module.
  Exit: the pass given the module's declarations (issue to file with the
  first user of an intrinsic created by a function pass).

## Used by
Named by `simplifycfg`.

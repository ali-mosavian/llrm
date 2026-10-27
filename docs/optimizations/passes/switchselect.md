> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/switchselect.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/switchselect.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/switchselect.rs sha256:2bfbd6ad3a853d260f827c809777b330dc0821d7cb145f2f59b0612a7cf12da3 -->

# switchselect

A `switch` that only picks the constant of one phi, turned into selects:
LLVM's
[`trySwitchToSelect`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L6972),
a step of `simplifySwitch`
([L8591](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8591)),
with `initializeUniqueCases`
([L6724](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L6724)),
`getCaseResults`
([L6644](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L6644)),
`foldSwitchToSelect`
([L6795](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L6795))
and `removeSwitchAfterSelectFold`
([L6925](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L6925)).

## How it works
Each case (and the default) is followed through blocks that only fold to
constants, to one common destination with one phi. With at most two
distinct results, at most 16 cases each, the switch becomes selects on
compares of the value, or on a mask when a result's cases are a bit group.

## Example
```text
  switch i32 %a, label %d [ 10, %x  20, %y ]   %c1 = icmp eq i32 %a, 20
  x: br %end   y: br %end   d: br %end       ==> %s1 = select i1 %c1, i32 2, i32 4
  end: %p = phi [ 42, %x ], [ 2, %y ], [ 4, %d ] %c0 = icmp eq i32 %a, 10
                                                 %s = select i1 %c0, i32 42, i32 %s1
```

Each case block may hold constant-foldable instructions on its way to the
common destination; at most two distinct results (and a default) are
taken.

| # | DEVIATE |
|---|---|
| D1 | No branch weights on the selects. |
| D2 | No names on the new values. |

## Used by
`switchopt::simplified`.

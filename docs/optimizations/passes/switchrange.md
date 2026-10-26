> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/switchrange.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/switchrange.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/switchrange.rs sha256:2286387e689b1e62139b3575bd057d9b7f8c5a87d7b51368ee5893cc51a13f4d -->

# switchrange

`turnSwitchRangeIntoICmp`, a step of `simplifySwitch`
([L6205](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L6205)),
with `findContiguousCases`
([L6124](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L6124)).

A switch with two destinations, one of which gets a run of consecutive
case values, becomes a range compare and a branch.

## How it works
The cases are split by destination, the default counting as the first. A
third destination ends the fold. A destination with one case, or whose
cases run without a gap (`findContiguousCases`), is the range: `x - min u<
count` (an `icmp eq` for one case) branches to it. When the value's range
spans exactly the first and last case of a set, the gap between the two
runs is the other destination's run. The phis of both destinations drop
the inputs of the cases that merged into one edge.

## Example
```text
  switch i16 %x, label %b [            %x.off = add i16 %x, -1
    i16 1, label %a                    %switch = icmp ult i16 %x.off, 3
    i16 2, label %a                    br i1 %switch, label %a, label %b
    i16 3, label %a ]
```

| # | DEVIATE |
|---|---|
| D1 | No branch weights. |
| D2 | No `DomTreeUpdater`. |

## Used by
`switchopt::simplified`.

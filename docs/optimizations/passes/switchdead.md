> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/switchdead.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/switchdead.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/switchdead.rs sha256:9015de9d10b341ccf390e7408393c027ebfe11386e35441ec672d039cbe7cc32 -->

# switchdead

Cases of a `switch` that cannot happen removed, and a switch over a
three-way compare made a branch: LLVM's
[`eliminateDeadSwitchCases`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L6365)
with `createUnreachableSwitchDefault`
([L6178](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L6178))
and `collectPossibleValues`
([L11052](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L11052)),
and
[`simplifySwitchOfCmpIntrinsic`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8242),
steps of `simplifySwitch`
([L8591](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8591)).

## How it works
A case is dead when the known bits of the value rule it out, it needs
more significant bits than the value has, or the value is one of at most
four constants (through `select` and `phi`) that it is not. With no case
dead, cases that cover every value the unknown bits allow make the
default unreachable, and one missing case is given to the default's
block. A `switch` on a `scmp`/`ucmp` with two destinations is a compare
and a conditional branch.

## Example
```text
  %c = and i32 %a, 3                   %c = and i32 %a, 3
  switch i32 %c, label %d [            switch i32 %c, label %d [
    i32 1, label %x                      i32 1, label %x
    i32 2, label %y                ==>   i32 2, label %y ]
    i32 5, label %z ]
```

| # | DEVIATE |
|---|---|
| D1 | No branch weights, no `DomTreeUpdater`, no assumption cache. |
| D2 | `collectPossibleValues` takes only integer, zero, null, undef and poison constants as constants. |

## Used by
`switchopt::simplified`.

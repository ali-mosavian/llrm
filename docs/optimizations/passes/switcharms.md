> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/switcharms.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/switcharms.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/switcharms.rs sha256:8c5e95fc394ab0533bb14135e834df65cfef481c371ed78cc27f48d90cd69e7d -->

# switcharms

`simplifyDuplicateSwitchArms`
([L8564](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8564)),
`simplifySwitchWhenUMin`
([L8027](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8027))
and `simplifySwitchDefaultBranch`
([L8077](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8077)),
the last steps of `simplifySwitch`.

## How it works
`folded` tries the three in LLVM's order and stops at the first that
changes the switch.
1. Arms that are a jump alone to the same block, with the same phi inputs,
   are one: the switch goes to the first (`mergeIdenticalBBs`, shared with
   `sinkcommon`).
2. A switch on `umin(a, C)` loses the cases above `C`. If its default is
   unreachable and it has the case `C`, that case becomes the default and
   the switch is on `a`.
3. If the switched value is a known constant where the default runs, that
   value gets a case to the default's block and the default becomes
   unreachable.

## Example
```text
  %c = umin i16 %a, 3                      switch i16 %a, label %b3 [
  switch i16 %c, label %unreachable [        i16 0, label %b0
    i16 0, label %b0                         i16 1, label %b1 ]
    i16 1, label %b1
    i16 3, label %b3 ]
```

| # | DEVIATE |
|---|---|
| D1 | No branch weights, no `DomTreeUpdater`. |
| D2 | Step 3 has no `AssumptionCache`, so only bits the value itself fixes are known. |
| D3 | Step 2 also removes the replaced default edge from the phis of the old default. |

## Used by
`switchopt::simplified`.

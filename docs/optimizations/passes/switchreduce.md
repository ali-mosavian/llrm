> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/switchreduce.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/switchreduce.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/switchreduce.rs sha256:549f15c66b8a0c7a9252a28bd48b3a32ca56a448fc69e5112f9a1e7977ac59df -->

# switchreduce

A sparse `switch` made dense: LLVM's
[`reduceSwitchRange`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L7925),
a step of `simplifySwitch`
([L8591](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8591)),
with `isSwitchDense`
([L7367](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L7367))
and `getDenseSwitchRangeReductionShift`
([L7390](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L7390)).

## How it works
With four or more cases on a legal integer of at most 64 bits, and the
sorted case values not already dense (`isSwitchDense`: 10% of the range
used, 40% at `optsize`), find a base and a shift that make them dense:
base 0 if dense at the optsize threshold, else the smallest case.

## Example
```text
  switch i32 %x [ 5, 9, 13, 17 ]    %s = sub i32 %x, 5
                                ==> %r = call i32 @llvm.fshl.i32(i32 %s, i32 %s, i32 30)
                                    switch i32 %r [ 0, 1, 2, 3 ]
```

A rotate right by the shift puts the bits shifted off in the top, so a
value that did not divide cleanly is large and takes the default.

| # | DEVIATE |
|---|---|
| D1 | No branch weights (the pin keeps the switch's own). |

## Used by
`switchopt::simplified`.

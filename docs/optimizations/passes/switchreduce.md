> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/switchreduce.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/switchreduce.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/switchreduce.rs sha256:36a5a29dd4152ec0d2f3f47660d3e2bf538bd519f998b1888d98fa4b8e4ff8d3 -->

# switchreduce

A sparse `switch` made dense: LLVM's
[`reduceSwitchRange`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L7925),
a step of `simplifySwitch`
([L8591](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8591)),
with `isSwitchDense`
([L7367](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L7367))
and `getDenseSwitchRangeReductionShift`
([L7390](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L7390)).

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

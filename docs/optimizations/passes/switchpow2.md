> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/switchpow2.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/switchpow2.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/switchpow2.rs sha256:44d7ead27d4cb70614832cab674d4717b6a6d33f0484ba960c45960c7e77b57f -->

# switchpow2

A `switch` over powers of two made dense: LLVM's
[`simplifySwitchOfPowersOfTwo`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8129),
a step of `simplifySwitch`
([L8591](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8591)).

## How it works
With four or more cases, all powers of two, dense once reduced to their
exponents (`isSwitchDense`), and a `cttz` the target prices at 2 or less
(`TCC_Basic * 2`, size and latency), the switch is on `cttz(x)` and each
case is its exponent. A reachable default is protected by a branch on
`ctpop(x) == 1` to the default, so a value that is not a power of two
never reaches the `cttz`.

## Example
```text
  switch i32 %x [ 1, 2, 64, 128 ]    %n = call i32 @llvm.cttz.i32(i32 %x, i1 true)
                                 ==> switch i32 %n [ 0, 1, 6, 7 ]
```

| # | DEVIATE |
|---|---|
| D1 | No branch weights, no `DomTreeUpdater`. |
| D2 | The block split off for a reachable default is placed last, not after the switch's block. |

## Used by
`switchopt::simplified`.

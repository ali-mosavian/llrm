> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/switchrange.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/switchrange.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/switchrange.rs sha256:4ac0cad9b61f3aa4312c06e4930f78773fed4e6a60f3ba1fcdeab76d298efeff -->

# switchrange

`turnSwitchRangeIntoICmp`, a step of `simplifySwitch`
([L6205](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L6205)),
with `findContiguousCases`
([L6124](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L6124)).

A switch with two destinations, one of which gets a run of consecutive
case values, becomes a range compare and a branch.

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

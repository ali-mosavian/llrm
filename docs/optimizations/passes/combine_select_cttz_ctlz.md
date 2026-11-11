> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_select_cttz_ctlz.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_select_cttz_ctlz.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_select_cttz_ctlz.rs sha256:d91f8d4de83db60430521290f5a68b610794fa5baf08c8f15a14d197b3fbe815 -->

# combine_select_cttz_ctlz

`foldSelectInstWithICmp`'s count-zeros step: a select that guards `cttz` or
`ctlz` against a zero input is the intrinsic with its zero case defined.

Translation of
[`foldSelectCttzCtlz`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L1561)
at d1106deb71cc, called from
[`foldSelectInstWithICmp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L2480).

MIRROR: an equality compare; the count, optionally under a `zext` or
`trunc`, is of `X` with `X == 0`, `X == -1` for `~X`, or `X == Y` for
`X ^ Y`; the arm taken at zero being the count's bit width gives the count
(its poison-generating annotations dropped); any other arm, with the count
and its extension each used once, relaxes the count to "zero is poison"
and drops its UB-implying attributes.

DEVIATE 1 (2026-11-09): the relax case ends the visit with `Changed`; the
pin keeps trying the later folds of `foldSelectInstWithICmp` on the same
visit. The select is revisited, so they run then. Exit: the fold list can
tell "changed, go on".

## Used by
`visitSelectInst`, after `canonicalizeSPF`.

## Example
```text
%c = icmp eq i32 %x, 0                       %r = call i32 @llvm.ctlz.i32(i32 %x, i1 false)
%z = call i32 @llvm.ctlz.i32(i32 %x, i1 true)
%r = select i1 %c, i32 32, i32 %z      ==>
```

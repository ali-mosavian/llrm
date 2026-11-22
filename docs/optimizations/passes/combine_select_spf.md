> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_select_spf.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_select_spf.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_select_spf.rs sha256:ad58be50488d870826d12b19d8b4bb4449c20736b5aa62d1b672b7d39eff3ee1 -->

# combine_select_spf

`foldSelectInstWithICmp`'s min/max steps: a select that is a min, a max or
an absolute value of a compare becomes the intrinsic.

Translation of
[`canonicalizeSPF`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L1623)
at d1106deb71cc, reached from
[`foldSelectInstWithICmp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L2419).

MIRROR: `matchDecomposedSelectPattern` of the compare and the two arms;
`smin`, `smax`, `umin` and `umax` for the min/max flavors; `abs` (poison at
the minimum when the other arm is the `nsw` negation of the first) for the
absolute value, with `0 - abs` for its negation; an absolute value is
skipped when neither the compare nor its negation has one use.

Also the clamp-away-from-an-extreme half of
[`foldSelectInstWithICmpConst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L2146)
(`x == 0 ? 1 : x` is `umax(x, 1)`, and the signed and `max` kin) and
[`foldSelectICmpMinMax`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L688)
(`x < y ? x : y - 1` is `smin(x, y - 1)`, `y + 1` is `smax`, with the
overflow flag of the other sign dropped; `(x <= y) ? 0 : x - y` is
`x - smin(y, x)`), each in the pin's order.

The sign-bit swap (`(X > -1) ? TV : FV` is `(X < 0) ? FV : TV`) is
translated, in the pin's place before `foldSelectICmpMinMax`.

DEVIATE 1: the other steps of `foldSelectInstWithICmp`
(the range half of `foldSelectInstWithICmpConst`, `canonicalizeClampLike`,
`tryToReuseConstantFromSelectInComparison`, `foldSelectICmpEq`) are not
translated; `foldSelectICmpMinMax` runs here, ahead of them.
Exit: translated by their own strips, by 2026-12-06 (owner x87, from wasm-3).

## Used by
`visitSelectInst`.

## Example
```text
%c = icmp slt i16 %x, 0                 %r = call i16 @llvm.smax.i16(i16 %x, i16 0)
%r = select i1 %c, i16 0, i16 %x   ==>
```

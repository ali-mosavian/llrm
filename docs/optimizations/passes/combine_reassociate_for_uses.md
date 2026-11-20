> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_reassociate_for_uses.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_reassociate_for_uses.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_reassociate_for_uses.rs sha256:bc7be116e8b79b68014f096b6eff527ee9a37490bb2b11fc91447c9f865e9568 -->

# combine_reassociate_for_uses

`reassociateForUses`: `(X op Y) op Z` with `X` or `Y` used elsewhere is
rebuilt so the one-use values share an instruction.

Translation of
[`reassociateForUses`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L2195),
called by `visitAnd`, `visitOr` and `visitXor`.

## Example
`down` is read again elsewhere, the two compares are not:

```text
%c = and i1 %down, %sign_ne
%r = and i1 %c, %mask_ne        ==>   %t = and i1 %sign_ne, %mask_ne ; %r = and i1 %t, %down
```
The compares now meet in one `and`, which the backend can chain with `ccmp`.

## Used by
Named by `combine_and`, `combine_or`, `combine_xor`.

> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/indirectbrexpand.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/indirectbrexpand.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/indirectbrexpand.rs sha256:4f927ef5806b31383a2ee35e435a9226069c0e35c41ed9e60cf45667f68d8809 -->

# indirectbrexpand

Replace `indirectbr` by a `switch` over block numbers, for a target that
has no jump through a register.

Pass: LLVM `IndirectBrExpandPass` (`CodeGen/IndirectBrExpandPass.cpp`).
Translation: each address-taken successor of an `indirectbr` gets a number
from 1 (0 is null, which block addresses may be compared with); its
`blockaddress` becomes `inttoptr` of the number, and the `indirectbr`
becomes a `switch` of the address as an integer, the first block the
default. Several `indirectbr`s share one `switch_bb` that takes the address
from a phi. A phi of a target block takes `undef` from a jump that did not
list it.
General solution: one IR pass makes computed goto compile on every target
whose backend does not select `BRIND`; the target asks for it with
`Tti::expands_indirect_br` (LLVM's `enableIndirectBrExpand`).

## Example
```text
indirectbr ptr %a, [label %one, label %two]
  ==>  %n = ptrtoint ptr %a to i16
       switch i16 %n, label %one [ i16 2, label %two ]
blockaddress(@f, %one) ==> inttoptr (i16 1 to ptr)
```

## See also
`simplifycfg` (folds a known address to `br`),
`llrm_mir::Context::replace_block_addresses`.

## Used by
Named by `pipeline`.

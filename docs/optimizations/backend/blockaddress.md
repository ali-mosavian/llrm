> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/blockaddress.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/blockaddress.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/blockaddress.rs sha256:2763a3933e8f1eb2209ec4168a07c08e1c626029b3a445c29f2efde7f49db34d -->

# blockaddress

The address of a block (`blockaddress`) as a symbol: one per address-taken
block, named by its function and its MIR block, so code and data in any
module order can name it before the function that holds it is lowered.

## How it works
The DAG carries it as a `GlobalAddress` of the function whose offset is
`offset` (a flag and the block), which every fold of a global plus an
offset keeps apart from a real offset into the function.
`GlobalMap::address` turns that into the `key` of the symbol;
`globals::names` names the key and the lowered function defines the name at
the block's label.

## Example
```text
blockaddress(@f, %b)  ==>  the symbol `$BA<f>_<b>`, defined at %b's label
indirectbr ptr %p, [%b, %c]  ==>  jmp *%p, successors %b and %c
```

## See also
LLVM `AsmPrinter::GetBlockAddressSymbol`, X86 `LowerBlockAddress`.

## Used by
Named by `builder`, `globals`.

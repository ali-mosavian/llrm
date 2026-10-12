> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/addressvalues.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/addressvalues.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/addressvalues.rs sha256:67afc3f4691f79af1e1e0b40bdb3ff7f3fafba05bcb8e69d9d68f289de00ecf8 -->

# addressvalues

Spells the `address` operations of an allocated body as `lea` operands.

## How it works
Before allocation an address-valued operation (`Operation::Address`)
names its memory cell as a `Mem` source. `lea` does not read memory, so
once registers are chosen each such source becomes an `Address` operand
(`[through + index_through*scale + offset]`) and no cell is read.

A frame cell with a base value becomes the frame register plus the cell's
register as index, at the cell's displacement. A cell with an address and
a base register keeps the address and takes that register as its base. Any
other cell keeps its address and registers. Instructions that are not
`Address`, and sources that are not `Mem`, pass through.

```text
 lea v9, Mem[frame-6, base v1, through si, scale 2]
   ->  lea v9, [bp + si*2 - 6]
```

## Used by
`assemble.rs`, on each machined body before `masm::cleaned_returns`, and
the BASIC driver (`driver/basic.rs`).

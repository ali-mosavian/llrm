> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/addressvalues.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/addressvalues.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/addressvalues.rs sha256:f3f9f64b925ae55c969ceebe1f1011851c9c75432b2aa532f8ae727866189823 -->

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

## Example
An `Address` operation whose source is a frame cell with a base value,
before and after (`Address` registers are numbered: 26 is bp, 27 is si):

Before:

```lir
lea v9, Mem(addr=frame-6, through=si, offset=3, disp_width=1, base=v1, scale=2), v4
```

After:

```lir
lea v9, Address(addr=None, through=26, index=27, scale=2, offset=-6, disp_width=1), v4
```

The frame register is the base, the cell's register the index, and the frame
displacement replaces the cell's offset.

From unit test `address_cells_convert_as_python_does` in `addressvalues.rs`
(first case, abridged from the asserted text).

## Used by
`assemble.rs`, on each machined body before `masm::cleaned_returns`, and
the BASIC driver (`driver/basic.rs`).

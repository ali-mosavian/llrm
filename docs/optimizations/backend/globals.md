> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/globals.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/globals.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/globals.rs sha256:c84cf41d4236d552f88e9394d2ba4cd37828ca4575424773466ddb3f9f5e1b4d -->

# globals

Turns a MIR module's globals into assembler symbols and data: names, and
each initializer's bytes with a relocation for every address in it.

## How it works
`names` maps every global to its assembler symbol, keyed as an `Addr` names
it (space, index). The name is what the ABI's `linked` makes of it, as
LLVM's Mangler does. An internal global whose name no assembler accepts
becomes `G$n`; an external one must already be a symbol, else it is an
error. An intrinsic has no symbol. A global outside the near address space
also gets a segment name, `seg <symbol>`, which a far pointer to it uses.

`datums` emits a defined variable as a label followed by its initializer,
walked as LLVM's AsmPrinter does: integers and floats as little-endian bytes
(x87 extended as its ten-byte format), zero and null as zero bytes, structs
with their padding, arrays member by member. Adjacent bytes merge into one
datum. An address constant becomes a relocation:

```text
 initializer constant              emitted
 --------------------------------  ----------------------------------
 ptr to global (+ GEP offset)      Pointer { name, offset }, near
 far pointer to a far global       Pointer { far: true }: offset+segment
 far pointer to near data          offset word, then DGROUP's selector
 addrspacecast to the segment      SegmentWord(symbol)
 ptrtoint of an address, i16/i32   Pointer { bytes: 2 or 4 }
```

A far pointer to near data is built from DGROUP's selector and the
group-relative offset, as a near pointer made far is: one POINTER fixup
would take the target segment's own selector. `target` resolves an address
constant to its global and byte offset through constant GEPs; `absolute`
resolves a constant that names no global (null, an integer made a pointer).

## Example
A C module on `-m16` with an array, a near pointer into it and a long.

MIR:

```mir
@_table = global [6 x i8] c"\01\00\02\00\03\00"
@_second = global ptr getelementptr (i8, ptr @_table, i16 2)
@_big = global [4 x i8] c"p\11\01\00"
```

Data the masm listing prints for it. The pointer is a `dw` relocation
against `_table`, plus the constant offset:

```asm
_table label byte
db 001h,000h,002h,000h,003h,000h
_second label byte
    dw _table+2
_big label byte
db 070h,011h,001h,000h
```

From `-S` and `LLRM_MIR_STAGES` of that module at -O2.

## Runs
`global names` and the data loop in `assemble::assembled_by`, once per
module after `nearcode`. Isel uses `space`, `target` and `absolute` for
address constants.

## Used by
`assemble.rs`, `isel.rs`.

## See also
LLVM `AsmPrinter::emitGlobalVariable`.

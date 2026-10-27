> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/constpool.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/constpool.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/constpool.rs sha256:fd20d7fd5a7643f6cfd28944c0384b2e22c97654b85d60cfdde29a3c4795bf21 -->

# constpool

Readonly constants the machine phases put in memory, one datum per
distinct bytes.

## How it works
The selector and the machine phases that need a constant no instruction can
encode ask the pool for
a cell holding its bytes. `Pool::cell` returns a readonly segment
cell; asking again for the same bytes returns the same cell, so a
constant appears once however often it is used. Ids start at
the module's global count, past the ids of its own data objects.
`narrowest` gives a float constant in `f32` bytes when that
is exact (or the value is NaN), else in `f64` bytes, as GCC's
`compress_float_constant`.

```text
  fild-style load of integer 5      fld  dword [$K12]
  (Imm, not 0 or 1)           =>    ...
                                    data:  $K12: 00 00 A0 40
```

After the last function is assembled the driver
(`assemble.rs`) walks `Pool::entries` in first-asked order, names each
datum `$K<id>` and emits it with the module's other readonly data.

## Example
`x * 2.5 + 1.25` on an f32: each constant is a pool cell (`seg:4`, `seg:5`)
in the selected LIR, and a datum in the listing. `; inserted` notes are
dropped.

```lir
v1 := fld [[bp+0x4]]
v2 := fld [[seg:4+0x0]]
v3 := fmul v1, v2
v4 := fld [[seg:5+0x0]]
v5 := fadd v3, v4
```

```text
$K4 label byte
db 000h,000h,020h,040h
$K5 label byte
db 000h,000h,0a0h,03fh
```

`$K4` holds 2.5 and `$K5` 1.25, as f32 bytes.

From `ISEL_DUMP=1 llrm-nib -O2 -fno-inline-functions
-fno-inline-functions-called-once -fno-ipa-cp-clone
-march=i386` of `fn scale(x: f32) -> f32`, stage `FarIndirectCalls` and
`-S`.

## Used by
`isel.rs`, for each float constant it loads, and `floatassign.rs`: an
integer other than 0 or 1 converted to float becomes a load of its float
constant (0 and 1 use `fldz` and `fld1`). The pool is shared by every
function of a module, so `$K` names are module-wide.

Tuned for size, `isel/matcher.rs` stores a double 0.0, -0.0, 1.0 or -1.0,
or a float 1.0, as `fldz`/`fld1` (and `fchs`) and an `fstp`
(`x87_store_pays`); a lone float 0.0 or -1.0 stays an integer store.

## See also
GCC `force_const_mem`; `crate::backend`

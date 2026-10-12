> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/constpool.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/constpool.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/constpool.rs sha256:3ae49d6f88c33f8e0d37f4e6f7975a34dcaeee56fb8d3b7bdc4d23ccaaada557 -->

# constpool

Readonly constants the machine phases put in memory, one datum per
distinct bytes.

## How it works
A phase that needs a constant no instruction can encode asks the pool for
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

## Used by
`floatassign.rs`: an integer other than 0 or 1 converted to float becomes
a load of its float constant (0 and 1 use `fldz` and `fld1`). The pool is
shared by every function of a module, so `$K` names are module-wide.

## See also
GCC `force_const_mem`; `crate::backend`

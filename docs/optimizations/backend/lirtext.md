> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/lirtext.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/lirtext.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/lirtext.rs sha256:024863ee4f13fdc40afc9cfad66d9908f8f5c0d3bb16fdc53c8a8ce7d721bfd3 -->

# lirtext

LIR as text, one stage at a time: what `ISEL_DUMP` prints after each
machine phase.

## How it works
`lir_stage` prints a `=== stage` line, then for each body its name and
instruction count, each block by address, its phis (`v3 := phi ...`) and
its instructions as `dests := name sources`, with `; notes` for what a phase
attached (`inserted`, `parallel-copy N`, `spill reload`, `frame adjust`,
`requires`, `delivers`, `clobbers`).
Addresses are hex with at least four digits, and the format is short enough
to diff between stages.

```text
=== PhiElimination
  f: 2 instructions
    block 0x0000
      0x0004  v5 := mov v2  ; inserted; parallel-copy 1
```

## Example
The first block of `main` after phi elimination:

```lir
=== PhiElimination
  main: 18 instructions
    block 0x0000
      0x0001  [[bp-0x4]] := mov 0x30000  ; inserted
      ; ...
      0x0002  v2 := mov v3  ; inserted
      0x0003  v1 := mov -7  ; inserted
      0x0003  v5 := mov v1  ; inserted; parallel-copy 1
      0x0003  jmp   ; inserted
```

The `v5 := mov v1` is the copy that replaced the loop header's phi.

From `ISEL_DUMP=1` of a nib program (a struct method with a `while` loop,
called from `main`) at `-m16 -O2`.

## Used by
`assemble` after each machine phase when `ISEL_DUMP` is set; the register
allocator's tests print input and result with it.

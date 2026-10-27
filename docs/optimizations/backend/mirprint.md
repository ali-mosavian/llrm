> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/mirprint.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/mirprint.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/mirprint.rs sha256:5ee9f5b893f1950c4e9af78c802f2493c336cd80003232966b5d5c9c3dc6d9a3 -->

# mirprint

Pre-allocation LIR as LLVM MIR, so the pin's `llc -run-pass=greedy`
allocates the same function (a like-for-like check of quality and cost).

## How it works
`mir` prints a body whose instructions are the 32-bit integer forms of
`mov`, `lea`, `add`, `sub`, `and`, `or`, `xor`, `neg`, `sar`, `shl`, `shr`,
`cmp`, `test` and the jumps, with registers, immediates, frame arguments and
indexed cells of a data segment. A value is a `gr32` virtual register; a
`requires` or `delivers` entry is a `COPY` from or to its physical register;
the data segment is a global array `@seg<n>`; the first incoming stack
argument is `%fixed-stack.0`. Anything else is refused with its name.

## Example
`v1 := mov [bp+8]; cmp v1, 0; jg` becomes
`%1 = MOV32rm %fixed-stack.0, 1, $noreg, 0, $noreg`,
`CMP32ri %1, 0, implicit-def $eflags` and `JCC_1 %bb.2, 15, implicit
$eflags`.

## Used by
`LLRM_MIR_PRINT=<function>` in the greedy shadow run;
`tools/mirprint/check.py`.

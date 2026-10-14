> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/machinecse.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/machinecse.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/machinecse.rs sha256:9a9e84542e7e691dec756f693505ae7a97c12964689e608ae6c7641fd7569242 -->

# machinecse

Removes a repeated pure register computation after allocation, by value
numbering the byte lanes each instruction reads and writes.

## How it works
MIR GVN sees values, not the physical instructions lowering and allocation
create. Here the question is physical: which bytes of which registers hold
the result of which computation.

A candidate is a `mov`, `movsx`, `movzx` or `lea` with one register
destination and register, immediate or address sources, whose result does
not depend on the destination's old contents and which sets no flags.
Memory reads are out (proving a cell unchanged is MIR's job), as are
relocated operands (each owns a fixup), calls, spill traffic and anything
with side conditions (`clobbers`, `requires`, `delivers`, groups).

Each lane maps to a token that names its value. A candidate's token is its
operation, its sources' encoded fields and the tokens currently in the
lanes it reads; the destination register is not part of it. If every lane
the candidate writes already holds the token it would write, the
instruction is redundant. Any other write gives its lanes a fresh token,
and an opaque instruction (call, return, barrier, block copy or fill)
clears the state.

```text
  lea  bx,[si+4]     bx.0..1 := V([si+4], si's tokens)
  ...                (si, bx untouched)
  lea  bx,[si+4]     bx already holds V: redundant
  lea  di,[si+4]     di.0..1 := V (same value, other register: kept,
                     destination placement is not part of the token)
```

Across blocks the incoming state is the tokens every predecessor agrees on.
Layout order is iterated to a fixed point; if it does not settle within
four rounds per block the body is returned unchanged. A redundant
instruction is replaced by an anchor (`lir::anchor`) that keeps its virtual
definition and source ownership but emits no bytes.

## Example
No honest example: it is one sub-pass of `peephole`, whose `ISEL_DUMP=1`
stage shows all the sub-passes' work together, and its tests build bodies by
hand and assert on the operations. A removed instruction prints in the dump
as an empty line (`0x0003     ; inserted`).

## Runs
A peephole sub-pass, once per `peephole` run, after `sharedstores` and
before the final `machinedce`.

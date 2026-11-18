> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/floatslotcopy.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/floatslotcopy.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/floatslotcopy.rs sha256:a9ad3c2c86cbdfcee25bf60a43663ab4d811287a6a7989b44e0c5d39f991f1fc -->

# floatslotcopy

A float copied from one cell to another is copied through a free general
register where that is cheaper than the x87 pair.

## How it works
`fld dword [A] ; fstp dword [B]` leaves the stack as it found it and moves
four bytes. A 486 takes 3 clocks for the load and 7 for the store; `mov
r,[A] ; mov [B],r` takes 1 and 1 per word. The pair becomes one such move
per register-sized word (a dword; a qword in long mode) of the cell where a
register is dead after the pair, the cells share no byte, and the clocks
saved, at the target's speed rate, outweigh the bytes added.

```text
 before                          after
 fld  dword [bp-4]               mov eax, [bp-4]
 fstp dword [bp-8]               mov [bp-8], eax
 10 clocks, 6 bytes              2 clocks, 8 bytes (m16: 66h prefixes)
```

The pair is a plain `fld` of a 4 or 8 byte cell
and the `fstp` of the same width that pops it next (anchors may sit
between). The price is the target's own (`x87_load`, `x87_store`, `mov_rm`,
`mov_mr`), the encoded sizes are the selector's. The clocks saved are
weighted by the block's runs a call (`LirBody::frequencies`, one where there
are none). Nothing is done where bytes are strict (-O1, -Os), where the
target has no speed rate, or where a cost is missing.

## Example
The ten-float rotation of `x87rotate.c` at -O2 -m16 spills the carried set
through frame cells; each shift of a cell is such a pair.

## Runs
A sub-pass of [`crate::backend::peephole`](peephole.md), once per body, after
`slotcopies::merged`.

## Keeps
None is returned where no pair qualifies.

## Deviations from the pin
- DEVIATE X87ROT-GPR-COPY (owner x87, review 2026-12-10): the pin leaves the
  pair (`llc -O2 -mcpu=i486` on the same IR keeps `flds`/`fstps` slot pairs
  and eleven `fxch` a trip); gcc and Watcom copy a spilled float through
  general registers. The bits move as they are: a signalling NaN is not
  quieted and no x87 exception flag is raised by the copy.

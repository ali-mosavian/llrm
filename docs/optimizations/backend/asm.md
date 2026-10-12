> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/asm.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/asm.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/asm.rs sha256:8db2aabaf72a79efadc1fb86d1b7b30b63038f28b5d36d1aa55b0f5e2213d967 -->

# asm

The types a laid-out body is handed over in: bytes, moved addresses and
relocations.

## How it works
`layout.rs` places the blocks, measures, shrinks branches to a fixed point
and emits; it returns a `Laid`. The types live here and `layout` re-exports
them.

- `Laid` is the code bytes, where each instruction ended up (`moved`, old
  address to new), each relocated displacement as (offset in `code`, the
  field's original address) so its fixup can follow, the fixups whose
  instruction no longer exists (`dropped`), the addresses a transform folded
  into a surviving instruction (`covered`; a branch never resolves through
  it), and the symbols.
- `Item` is an instruction to encode (`Op`) or a `Table`.
- `Table` is a run of bytes between instructions that is copied, not
  selected: an `ON GOTO` table of a count byte and one relocated word per
  target. Only its fixups move.

`dropped` is reported rather than omitted so `objbuild` can tell a fixup
meant to go from one it cannot place, which it refuses.

## Used by
`layout.rs`, `objbuild.rs`.

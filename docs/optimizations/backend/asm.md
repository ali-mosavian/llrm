> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/asm.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/asm.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/asm.rs sha256:217908c83117fc514ee988d4645d870b23ed914ef4f7a4f3315ce558432d4c62 -->

# asm

The result of code layout: a body's bytes, where each instruction landed and
the relocations that must follow, as the object writer needs them.

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

## Example
No honest example: the module declares types and has no behaviour. Read
`layout.rs`, which builds a `Laid`, for the bytes and fixups of a real body.

## Used by
`layout.rs`, `objbuild.rs`.

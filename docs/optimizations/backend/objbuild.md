> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/objbuild.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/objbuild.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/objbuild.rs sha256:c21419ae2928fae719c9acaec2933195fa2d4b0b9b6387987eb14cff4d01dd71 -->

# objbuild

Encodes a masm module as a format-neutral object: code and data laid out in
sections, each name resolved, each reference to one a relocation.

## How it works
`built` drops the data objects nothing reaches (`live`), then makes one
segment per kind: code (one for the module, or one per procedure with
`CodeLayout::PerProcedure` so the linker can drop uncalled procedures),
each named data segment, and a stack segment if the module asks for one.

Data (`_data`) is written first, recording the offset of each label. Code
(`_code`) takes each procedure's items from `masm::listing`, encodes the
instructions (`_items`, through the target's encoder) and leaves branches
as jump items of unknown length. Their lengths are settled before any byte
is placed.

```text
  items  : piece  jump(L)  piece  label L  piece  near-call(f)
  pass 1 : every jump short (2 bytes)
  pass n : a jump whose target is out of 8-bit reach grows; repeat
  fixed  : growth only moves targets further, so it ends, at the smallest
           layout
```

`_relaxed` does that fixed point, as jwasm does. When the procedures are
tuned for size, `_trampolined` follows it: a conditional jump still long
(on a 386 `jcc rel16`, 4 bytes) whose target label has an unconditional
`jmp` within short reach is aimed at that `jmp` instead, saving two bytes
at the price of a taken `jmp` (Watcom's `SetBranches`), and the fixed point
runs again.

A call to a procedure of the same segment is a relative displacement filled
in; any other is a relocation. `object_of` turns what the segments hold
into `llrm_object::Object`: a reference to a symbol the module defines
becomes a relocation against that symbol with the addend in the code (as
jwasm writes it), anything else is an undefined symbol, and a reference to
a name neither defined nor declared is an error. 16-bit data segments of
the near kind join the `DGROUP` group.

Code is padded as MC does: an `Align` item (a block's `align` and
`max_skip`) pads with `nops` (`writeNopData`), and each function not tuned
for size starts at the target's preferred function alignment: `.p2align` in
OMF and COFF, ELF's `.prefalign` (no more than the body fills).

`written_in` hands the object to the writer crate of the target's format
(OMF, ELF32/64, COFF32/64, Mach-O x86-64), or refuses a pair none writes.

## Example
No honest example: its output is the bytes and relocations of an object, not
LIR. `-S` prints the items it encodes (see `masm`); `tests` in this file
assert the bytes.

## Runs
Once per module, after `assemble` and `data layout` have produced the masm
module, when a frontend writes the object (`llrm-cc`'s `compile.rs` calls
`written_in`). The BASIC driver (`driver/basic.rs`) reuses `_data`,
`_code_by` and `object_of` with its own listing and header.

## See also
[`crate::backend::masm`](masm.md) for the items, `asm` for the encoder.

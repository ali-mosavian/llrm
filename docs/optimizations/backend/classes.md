> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/classes.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/classes.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/classes.rs sha256:65a8e14744dc8c26b60aaa34b843b35b48d39ae57d39d090cbcc6588aa4984b7 -->

# classes

What the selected target says about its registers, for the allocator and
the passes around it.

## How it works
`RegisterClasses::of` reads the target's register file and its
instruction forms once. From the register file: the registers a value may
sit in (`available`, in allocation order), the word registers an address
is made of (`word_bases`, `word_indexes`, `addressing`,
`encodable_bases`), the frame register, and the words whose low byte has
a register of its own (`byte_words`). From the forms' `fixed` column:
which operand of which form lives in which register (`requirements`),
such as a divide's `ax` and `dx` or a shift's `cl`.

A pin that only tells members of a family apart (`les`, `lds`, `lfs` and
`lgs` take the same operands and differ in the selector) is a choice the
allocator makes, so it is left out. A pinned immediate is no register, and
a segment register is pinned only while it is a held value.

`with_frame_free` is the same classes for a function with no frame
register: the frame register becomes one more general register, last to
be given out, and an address may be made of its word.

## Example
What `RegisterClasses::of` says of the real-mode target:

```text
addressing   BX BP SI DI
status_word  AX
```

From unit tests `m16_registers_are_its_description` and
`the_status_word_is_where_the_form_table_stores_it` in `classes.rs`.

## Used by
`allocate.rs`, `coalesce.rs`, `constrain.rs`, `regclass.rs`, the spiller
(`ssaspill.rs`), `pressuresink.rs`, `floatalloc.rs`, `sharedstores.rs`,
`loopslots.rs` and `peephole.rs`. `assemble.rs` builds them per module and
`flow.rs` hands them to each phase beside `Segments`; no pass reads
another target's.

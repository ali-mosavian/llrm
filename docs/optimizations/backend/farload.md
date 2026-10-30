> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/farload.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/farload.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/farload.rs sha256:3346d3b9e2d93ebd04919abad6ee8917a37ee660d41ad0230b1c0d5f952cf8a3 -->

# farload

Select a far pointer's two word loads as one `les`-form load.

## How it works
A far-pointer field is two word loads in the language but one
instruction on the target. Once allocation has put the offset and the
selector in unrelated registers the pair can no longer be joined, so
this runs first, on virtual registers.

```text
  mov  v1, [p]                     les  v1:v2, [p]
  mov  v2, [p+2]           =>      (second load becomes an anchor)
                                   ... [v2:v1] used as a far cell
```

`selectors` first finds the values that are read only as a memory cell's
selector: never as a number, a register argument, a phi input or a base
or index. Only such a second value may become the far segment register.

`selected` then walks adjacent instruction pairs. Both must be plain
`mov reg,[mem]` loads (no clobber, constraint, group, relocation,
spill marker or volatile access), the second cell must be the word right
after the first (same registers, displacement or offset 2 higher), the
second destination must be in the selector set, and the cell must
name at least one virtual register (a fixed address survives allocation
unchanged) that neither load defines. The pair becomes one four-byte
load named by the target's far-segment load form; the second load
turns into an anchor so source ownership is kept.

## Example
`world->words[i]` and `rdr->flags[...]` load far pointer fields of near
structs. No dump prints the body before this pass, so this is its result:
each pointer is one `les` defining the offset and the selector (`v11, v12
:=`), followed by the anchor the second word load became. The first stage
dump of `mark`:

```lir
_mark@3: 26 instructions
  ; ...
  block 0x0007
    ; ...
    0x0009  v11, v12 := les [None v1@unplaced]  ; inserted
    0x0009     ; inserted
    0x000b  v13 := mov [[es:r0+0x0] v11@unplaced]  ; inserted
    0x000c  v14, v15 := les [None v2@unplaced]  ; inserted
    0x000c     ; inserted
```

From `ISEL_DUMP=1` of `tests/fixtures/c/farloadloop.c` with `-m16 -O2`.

## Runs
Per block in isel's cross-instruction selection (`isel/combined.rs`),
first, before [`crate::backend::comparefold`](comparefold.md). Every optimization level.

## See also
The phase map: `crate::backend`

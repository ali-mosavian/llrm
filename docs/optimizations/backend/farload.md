> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/farload.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/farload.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/farload.rs sha256:5cfb9d72da7477b758a46b664802531d9ac806393900a4ead37c67b2009a8250 -->

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
each pointer is one `les` defining the offset and the selector (`t11, t12`),
followed by the `elided` line the second word load became. The first stage
dump of `mark`:

```lir
═══ _mark@3 · FarIndirectCalls · m16 ═══════════════════════════════════════════
; ...
    B2  ← B2 B3  → B2 B4  loop                                        ×31.0
         ; ...
         #8  t11, t12   = les.i16     [t1+38]:i32
         #9               elided
        #10  t13        = load.i16    [t12:t11+lsr.iv]
        #11  t14, t15   = les.i16     [t2+1014]:i32
        #12               elided
```

From `ISEL_DUMP=1` of `tests/fixtures/c/farloadloop.c` with `-m16 -O2`.

## Runs
Per block in isel's cross-instruction selection (`isel/combined.rs`),
first, before [`crate::backend::comparefold`](comparefold.md). Every optimization level.

## See also
The phase map: `crate::backend`

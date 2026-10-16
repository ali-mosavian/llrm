> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/farcall.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/farcall.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/farcall.rs sha256:bdece5bbdf93bacd8789bc5778537b420dd9de97190ffd300b75b077b9c312cf -->

# farcall

Give a far indirect call the memory operand the 16-bit ISA requires.

## How it works
A 16:16 far call has only an `m16:16` form, while MIR carries the code
pointer as one four-byte value in a register. The pass stores that value
into one frame cell and calls through the cell.

```text
  call  t5                 store [slot0], t5
  (indirect, 4 bytes)  =>  call [slot0]
```

Only an indirect call whose single source is a four-byte value counts,
and only on a target with a far segment register; elsewhere a dword is a
near pointer and the call is left alone. All such calls in a body share
the one cell (`("far-indirect-call", 0)`, four bytes), made on first use.
The call stops reading the pointer's value but keeps reading
its register arguments. This is form selection, not spilling: the store
is not a spill and the allocator treats the cell as any frame slot.

## Example
`int apply(int (__far *f)(int), int x) { return f(x) + 1; }` on `-m16`. No
dump prints the body before this phase, so this is what it leaves: the
pointer is stored to the frame cell `slot0` (as two halves) and the call
reads the cell.

```lir
═══ _apply@3 · FarIndirectCalls · m16 ══════════════════════════════════════════
fn _apply@3(t3@ax) -> @eax   ; calls clobber eax ecx edx ebx es fs gs
frame  slot0 [bp-4] 4 B  arg0 [bp+0] 10 B

B0  entry  → exit                                                 ×1.0
     #2  t1         = load.i16    [arg0+6]
     #3  t2         = load.i16    [arg0+8]
     #4               store.i16   [slot0], t1
     #5               store.i16   [slot0+2], t2
     #6  t4@eax     = call        [slot0](t3@ax)
     #7  t5         = add.i16     t4, 1
     #8               ret         t5@eax
```

From `ISEL_DUMP=1` of that function, stage `FarIndirectCalls`, at -O2.

## Runs
First machine phase (`far-indirect-calls`), before `ssaspill`, once per
body in `machine_with` (`flow.rs`). Every optimization level.

## See also
The phase map: `crate::backend`

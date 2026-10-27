> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/farcall.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/farcall.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/farcall.rs sha256:788c0c4859502185b3f2b915a6e28df0440ffa8d66b36cabdd68c4e7ebeee53e -->

# farcall

Give a far indirect call the memory operand the 16-bit ISA requires.

## How it works
A 16:16 far call has only an `m16:16` form, while MIR carries the code
pointer as one four-byte value in a register. The pass stores that value
into one frame cell and calls through the cell.

```text
  call  v5                 mov  [bp-0x14], v5
  (indirect, 4 bytes)  =>  call [bp-0x14]
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
pointer is stored to the frame cell `[bp-0x4]` and the call reads the cell.

```lir
_apply@3: 8 instructions
  block 0x0000
    0x0000     ; inserted; delivers v3@AX
    0x0000  v1 := mov [[bp+0x6]]  ; inserted
    0x0000  v2 := mov [[bp+0x8]]  ; inserted
    0x0000  [[bp-0x4]] := mov v1  ; inserted
    0x0000  [[bp-0x2]] := mov v2  ; inserted
    0x0000  call [[bp-0x4]]  ; inserted; requires v3@AX; delivers v4@EAX; ...
    0x0001  v5 := add v4, 0x1  ; inserted
    0x0002     ; inserted; requires v5@EAX
```

From `ISEL_DUMP=1` of that function, stage `FarIndirectCalls`, at -O2.

## Runs
First machine phase (`far-indirect-calls`), before `ssaspill`, once per
body in `machine_with` (`flow.rs`). Every optimization level.

## See also
The phase map: `crate::backend`

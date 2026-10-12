> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/farcall.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/farcall.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/farcall.rs sha256:037e6e54a95d7eb3b6067457ab43eb8b0b3daf64000f2c5e1f4e0bd6ba5e976d -->

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

## Runs
First machine phase (`far-indirect-calls`), before `ssaspill`, once per
body in `machine_with` (`flow.rs`). Every optimization level.

## See also
The phase map: `crate::backend`

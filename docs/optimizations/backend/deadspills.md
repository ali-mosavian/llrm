> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/deadspills.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/deadspills.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/deadspills.rs sha256:08482ddeeecc72180fb2e35f1f2f1bb7a009485f7ffb3e6874de0b27397be18f -->

# deadspills

Drops a spill store no later read of its slot can see. `unread_spill_stores`
drops the stores to a slot nothing reads; a slot several values share is
read, yet a store to it can still be overwritten on every path before the
next read:

```text
L0:  mov [bp-6],dx      ; dead: L1 stores [bp-6] before it reads it
L1:  mov [bp-6],si
     mov ax,[bp-6]
```

DEVIATE: the pin's greedy allocator would not share the slot
(StackSlotColoring only merges non-overlapping live ranges, so its store is
read); ours shares homes (#6096) and leaves the dead store. Exit:
pin-faithful spill placement reaches the same shape; this pass then finds
nothing and goes.

Backward liveness of the slots over the successors; a plain store kills its
slot, any other operand naming it reads it. Only a slot the allocator owns
(`spill_homes`) whose every access is the same cell is judged: a slot
another cell overlaps, or that a pointer can reach, stays.

## Example
The two blocks at the top: `mov [bp-6],dx` in `L0` is dropped, `L1`
stores the slot before it reads it. From unit test
`test_a_spill_store_the_next_block_overwrites_before_a_read_is_dropped` in
`peephole_tests.rs`.

## Used by
Named by `peephole`.

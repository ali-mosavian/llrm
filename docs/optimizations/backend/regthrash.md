> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/regthrash.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/regthrash.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/regthrash.rs sha256:fc7725dfdbaa2d7da69f571cec5c7e2da2f04bc72db60da1a23365fca2ba988b -->

# regthrash

Port of `qbopt/backend/regthrash.py`: renaming a definition's register so
a copy after it is redundant.

Open Watcom's `RegThrash` (`bld/cg/c/scthrash.c`), and it is the inverse of
coalescing. Registers are already chosen, so it takes

```text
OP(...) -> Y        ...        mov Z, Y     (and Y dies there)
```

and writes the producer's result into Z instead, which leaves the move
copying Z to itself. Between the definition and the move, Y has to stay
live and untouched and Z has to be dead, and the rewritten instruction has
to still exist: `select.emit` is that check here.

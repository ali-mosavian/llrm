> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/regthrash.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/regthrash.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/regthrash.rs sha256:01fe1883d812ca6c267f166740d8e775829b269379ef1872a482bdcfd99fc714 -->

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

# Multiway branches

For frontend authors and contributors to `decide`, `dfajumpthreading` and isel. A multiway branch is one MIR instruction, so a frontend states the dispatch and the optimizer sees every case.

## MIR `switch`

`switch` takes one integer selector, ordered `(value, block)` cases and a default. Destinations may repeat; duplicate case values may not. Passes that read it:

- `decide` ([`decide.rs`](../../optimizations/passes/decide.md)) selects the case a known selector takes.
- `jumpthreading` threads a switch's edges and `dfajumpthreading` the switch of a loop's state ([levels](../levels.md)).
- `loopclone` remaps cases and default, keeping successor phis; `loopmotion` and `peelsize` read it.
- isel ([`isel.rs`](../../optimizations/backend/isel.md)) expands it to compares and branches; a case equal to the default needs no compare.

```
 switch i16 %sel, label %d [ 1: %A  2: %A  3: %d ]
        |  isel
        v
   cmp ax,1 ; je A
   cmp ax,2 ; je A
   (default: jmp d unless it is the fall-through)
```

## BASIC `ON GOTO`

BC compiles `ON expr GOTO` to `B$OGTA` followed by an inline table of code offsets. The raise ([`emit.rs`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/target/llrm-x86-bc/src/emit.rs#L622)) reads the table and emits a guarded dispatch instead of the call:

```
 %above = icmp ugt i16 %bx, 255
 br %above, dispatch.error, dispatch
 dispatch.error:  call @llrm.qb.<error>(Illegal function call) ; unreachable
 dispatch:        switch %bx, label %past [ 1: t1  2: t2 ... ]
```

A selector of 0, or past the count, goes on past the table; past 255 is error 5, as the runtime does. The recognition applies only to the audited QB, PDS and VBDOS runtime routines in `INLINE_TABLE` and refuses a table that names code outside its segment. `ranges` removes the `> 255` test where the selector's interval excludes it.

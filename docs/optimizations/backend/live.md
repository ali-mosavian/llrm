> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/live.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/live.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/live.rs sha256:735cd500d08e04b35c3614296082ba70e079b4fa021a94891519bd3c46410fd3 -->

# live

The live intervals the allocator holds of the body it is rewriting, and the
spill weight each value gets from them.

## How it works
An interval is worked out once and kept while the instructions that name
its value stay. Instruction numbers are stable keys (`Indexes::patched`), so
a rewrite moves none of them. What a rewrite does change is the size of an
interval that an added or removed instruction lies inside, and with it the
weight, which is a function of the size. The weight is therefore not stored
but worked out when it is read, from what it is made of.

```text
            total        sibling    fold
  weight = (-------  -  -------  - -----)  floored at 0
            size         size       size

  size   = slots the value is live for + GRACE   (Interval::spill_size)
  pinned = INF                                    (never spilled)
```

`total` is the weighted references the value has. `sibling` and `fold` are
what its copies and its foldable loads save if it stays in a register; they
come from the allocator (`priced`). A value in `pinned` (a spiller product,
or one the caller protects) weighs infinity.

After an edit, `LiveRanges::edited` drops the intervals of the touched
values and puts the recomputed ones in; the rest are shared by `Arc`. The
allocator also leaves its latest intervals in a thread-local (`hold`); the
spiller it calls takes them back (`held`) when the body is exactly the one
they were made of, instead of working them out again.

## Used by
`allocate` builds and edits it; `spiller`, `splitkit`, `regclass` and
`liveunion` read it (through the `Ranges` trait where only the intervals
matter).

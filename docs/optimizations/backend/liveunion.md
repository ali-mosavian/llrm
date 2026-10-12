> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/liveunion.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/liveunion.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/liveunion.rs sha256:176bbcb95faa44f597fc48b9fbdbe06223fee39f3255f4a4dcb2e049a913daee -->

# liveunion

What each register holds, by where: LLVM's `LiveIntervalUnion` and the
`LiveRegMatrix` over it.

## How it works
The union keeps, per register, the values that hold it in the order they
took it. A query asks which holders meet an interval. With few holders it
looks at them one by one. Once a register has `BIG` holders and `ASKED`
questions have come since the intervals last moved, the register gets an
index built from where its holders are live.

A register's holders normally do not overlap, so the index keeps their
segments in an ordered map from start to (end, holder). A query walks the
segments that start before the interval ends, back to the first that ends
before it starts: logarithmic, and it visits the overlaps found rather than
the holders there are.

```text
  index of one register
    kept    start -> (end, holder)   segments that meet no other kept
    points  start -> holders         segments of no length
    apart   [(start, end, holder)]   segments that meet one already kept
```

A rewrite moves every interval and leaves the clashes for the allocator to
find, so the holders can overlap. A segment that meets one already kept goes
to `apart` and is compared one by one, so the answers equal the pairwise
ones in every case. Zero-length segments are points, because a segment of
no length still meets another it lies inside.

`refresh` drops the indexes after the intervals move; `refresh_among` moves
only the changed values. `check_index` asserts every index against the
pairwise answer.

## Used by
`allocate` and `splitkit`.

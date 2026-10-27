> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/liveunion.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/liveunion.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/liveunion.rs sha256:a687030e03ce27f0cb4ddb2476ecffef54bd474b219ff4c18ef3654b8b0cef2c -->

# liveunion

Answers which values already hold a register where an interval lies, so the
allocator can test an assignment without visiting every holder. LLVM's
`LiveIntervalUnion` and the `LiveRegMatrix` over it.

`Placed`, here too, is the allocator's table of where each value is: a
register by value, with the order the values were placed in kept as a number
so that taking a value out of the middle shifts nothing.

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

## Example
No honest example: the index answers queries inside the allocator and prints
no LIR. Read `tests` in this file, which assert every index against the
pairwise answer (`check_index`).

## Used by
`allocate` and `splitkit`.

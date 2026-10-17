> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/interferes.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/interferes.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/interferes.rs sha256:ab02f68f0392e63b527ebc6ebeb1555ee8eeda59fb44c2f684673641315c302f -->

# interferes

Whether two values interfere, asked of the pairs a pass has a question
about. LLVM structure: `LiveRange::liveAt`
([`LiveInterval.h#L411-L414`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/LiveInterval.h#L411-L414))
and `LiveRange::overlaps(other)`
([`#L458-L462`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/LiveInterval.h#L458-L462))
asked of two registers' ranges, with `overlaps(other, CP, Indexes)` letting
a copy's two ends meet. Ours: the same question asked of the values'
occurrences and segments, because the ranges are not kept (DEVIATE until
`liveness::LiveRange` lands: then this is `overlaps_cp`);
`coalesce::_interference_among` builds the graph of every pair of the asked
values live together (quadratic in the values live at once, a row of bits
per value for each block they cross), which this replaces for the pairs
asked.

## How it works
Two values interfere where one is defined while the other lives on (a copy
between values of one width does not make its source and destination
interfere), where both are live together after or before a parallel copy
(which reads all its sources before it writes any destination), and at the
entry of a block nothing branches to. The first is a point query on the
values' occurrences; the others are found from the values' live segments
(numbers `base[block] + n` for the point before instruction n) by a search
for a copy or entry point inside both. A body with phis is the graph's to
answer. `LLRM_CHECK_TWOADDR` compares each answer with the graph's.

## Example
`a` is defined at 3 and read at 9, `b` is defined at 5: `b`'s definition is
inside `a`'s range, so they interfere; had `b` been defined at 10 they would
not. If `b := a` is a copy of one width at 5, they do not either: `b` is
`a` there.

## Used by
`twoaddr` (which source of a commutative operation to tie). The coalescer's
graph users move here when the coalescing policy is decided.

> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/interferes.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/interferes.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/interferes.rs sha256:c972773642a3c6966f28a95b5ab347be7a7bbfae9063f37aae4bf5be2880fd97 -->

# interferes

Whether two values interfere, asked of the pairs a pass has a question
about: LLVM's `LiveInterval::liveAt` and `LiveRange::overlaps` asked of two
registers, where `coalesce::_interference_among` builds the graph of every
pair of the asked values live together (quadratic in the values live at
once, a row of bits per value for each block they cross).

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

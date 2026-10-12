> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/pressuresink.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/pressuresink.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/pressuresink.rs sha256:d78e91752ad6dfab1898ebf74fc8d86dbc933be782429a6686c291d7b0b349bc -->

# pressuresink

A load made where its one reader is, when that raises the pressure of no
register class: LLVM's scheduler works bottom-up for the same reason, a
value defined late is live a short while. Before allocation, where each
value's class is what `regclass` says confines it (a byte register, an
address base or index, a selector) or the x87 stack, and the loads of
different classes are told apart by nothing but those sets.

Moving `d = load [a]` down to its reader `u` takes `d` out of its class over
the stretch, and keeps each address operand that died at the load live to
`u`. Per class `S` that changes the count of live values confined within `S`
by `-[d in S] + #{operands in S that die at the load}`. The load moves where
no class rises and one falls.

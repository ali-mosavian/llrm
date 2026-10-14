> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/placed.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/placed.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/placed.rs sha256:64ad0311aae6e859a45785016de12fca72434d42be1ffb3ac97d69d3fe181a48 -->

# placed

Where the allocator has put each value: a register by value, and the order
the values were put in.

An insertion-ordered map pays for taking a value out of its middle with a
shift of everything after it (`IndexMap::shift_remove`: 14% of the compile
of `branches` N=800, where last-chance recoloring takes out and puts back
the holders of a register at every try). The order is kept as a number each
value was given when it was put in, and made a list only where it is asked
for: the end of an allocation, where values that share a copy are moved to
join it in the order they were placed.

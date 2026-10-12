> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/spill.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/spill.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/spill.rs sha256:b9cdcdad9a04e56358c60846eb8a1789ed831abd839b94a8208b866877b36778 -->

# spill

What values past the target's registers cost: the one spill model every
pricing of register pressure shares.

A value kept in memory costs its traffic there: a store after each
definition, a load at each read, and a memory update each time a counter
is stepped in place; a value its one instruction rebuilds for less than a
load costs that at each read instead. A counter and its step share one
cell, as a copy-joined web does, and a phi's own edges into that cell
move nothing. Where more values live than the target's registers hold,
or than a call leaves across it, the cheapest are spilled, and stay
spilled.

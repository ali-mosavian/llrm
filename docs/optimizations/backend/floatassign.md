> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/floatassign.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/floatassign.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/floatassign.rs sha256:0d18b3925f75ef4ebb6fde58a188c466bc1810301bbb162f8ae1ae89d2814337 -->

# floatassign

Pass one of GCC's x87 allocation, as IRA and LRA do for any register
class: which floating values hold one of the eight flat stack registers,
and where. Every spill, restore and memory operand comes out
explicit, so `floatalloc` converts to stack form and decides nothing, as
reg-stack does.

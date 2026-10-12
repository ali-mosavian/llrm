> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/addresssink.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/addresssink.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/addresssink.rs sha256:11942ebaa547503f2c082e3425ddc1c9d0557e39c9594c623580e055f00131cb -->

# addresssink

CodeGenPrepare's `sinkCmpExpression`: a comparison a branch in another block
reads is made again beside that branch, so isel keeps it in the flags (a
loop's test, GVN'd with the copy of it made ahead of the loop, was an `i1`
held across the loop's body: `sete`, a store of the byte, `cmp` and `jne`,
hanoi).

A phi read only as an address, whose arms each compute the same thing of
values the join sees, is that computation made once in the join: GVNSink
and CodeGenPrepare's address sinking, so that isel matches one addressing
form for the access.

`a(i) = ...` under an `IF` computes its element's address in each arm, and
the store after the join reads a phi of equal addresses: a register held
across the arms where `A%+disp[bx]` would do (RGBLIGHTS, #386).

It runs last, with `gepoffset`: until then gvn's partial redundancy
elimination makes exactly this phi, so that a load after the join reads
what the arms stored, and a sink earlier would be undone by it.

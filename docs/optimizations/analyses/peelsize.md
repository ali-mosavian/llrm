> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/peelsize.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/peelsize.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/peelsize.rs sha256:17ed70c250b4b5e3d8b119cec4710b831207e70141ac98bb16e7c5756fa6cf6c -->

# peelsize

The budgets the mid-end's passes read: `Limits`.

The GCC-model `admitted` (is a complete copy of a counted loop worth it) was
deleted with `Options::peel`, `Limits::grows` and the trip and operation caps
only it read: nothing called it outside its tests, and the translated
`LoopUnrollPass` decides the copies.

## Used by
Named by `pipeline`, `target`.

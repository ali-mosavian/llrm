> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/prezeroed.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/prezeroed.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/prezeroed.rs sha256:79f593026162366993d85da6910b9e8017eb4f0d8308d536808f436a14d287f2 -->

# prezeroed

The entry block's zero writes to a local nothing has touched yet, for a
frame the runtime zero-fills (QB's `B$ENRA`): the frame already holds them.
Not the pin's (it has no such runtime); both selectors drop the same set.

DEVIATE SD222: the DAG path drops these writes as the old selector does;
the pin has no runtime that zero-fills a frame. Owner w12. Exit: none
(a QB runtime fact). Review 2026-12-10.

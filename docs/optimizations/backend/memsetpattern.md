> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/memsetpattern.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/memsetpattern.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/memsetpattern.rs sha256:1a4a374b03fd8b0df274a9830670d385257b2c2aad59417a96b135b4810c6ebb -->

# memsetpattern

`llvm.experimental.memset.pattern` as a store loop, before selection. The
expansion is `llrm_transforms::memsetpattern`; this module says which
selectors take it.

## Example
`--isel=dag` takes the loop; `--isel=old` selects the intrinsic itself.

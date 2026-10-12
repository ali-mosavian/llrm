> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/globalopt.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/globalopt.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/globalopt.rs sha256:950598de983d2db890d01136b67abb985b77101f368d65ab1d3859c47eaeb48c -->

# globalopt

An internal global the module only ever loads from made `constant`, as
LLVM's GlobalOpt marks one it finds never stored: a load of it is then
its initializer's bytes (`memory::constant_bits`). The old BC raise
exposed BC_CN's loader bytes at the program's entry
(`raising_literals`); a global's initializer is those bytes here.

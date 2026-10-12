> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/globalopt.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/globalopt.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/globalopt.rs sha256:c033c5c641bc48256515ad47873b9ce77f5d250868c2fe64061f5409a693a5cf -->

# globalopt

An internal global the module only ever loads from made `constant`, as
LLVM's GlobalOpt marks one it finds never stored: a load of it is then
its initializer's bytes (`memory::constant_bits`). The old BC raise
exposed BC_CN's loader bytes at the program's entry
(`raising_literals`); a global's initializer is those bytes here.

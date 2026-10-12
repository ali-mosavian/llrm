> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/globaldce.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/globaldce.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/globaldce.rs sha256:c1b696432eb509d47630d85d7dd4dc6ff85d22e71237bfad671901730c61ad73 -->

# globaldce

LLVM's GlobalDCE: a module's own body or variable that nothing live
names -- no code outside the program, no live body, no live initializer
-- is deleted. A declaration stays: naming one asks the linker for it.

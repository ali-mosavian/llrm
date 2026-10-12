> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/globaldce.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/globaldce.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/globaldce.rs sha256:e5ee162882a9edab42699e19d5bb46fd81a931c8a870fbbc8eaa71899fa82708 -->

# globaldce

LLVM's GlobalDCE: a module's own body or variable that nothing live
names -- no code outside the program, no live body, no live initializer
-- is deleted. A declaration stays: naming one asks the linker for it.

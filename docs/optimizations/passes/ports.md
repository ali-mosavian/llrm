> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/ports.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/ports.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/ports.rs sha256:30ec7c5067617872e961ee2768553b4325940946abcac61846fe38fd53bb49bd -->

# ports

Each I/O port call narrowed to the memory its device reaches, as the
target describes the port. One with no path to memory touches only its
own state, `inaccessiblemem`, which keeps port calls in order. A port
that is not a constant is narrowed where `ranges` bounds it.

> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/asm.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/asm.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/asm.rs sha256:d8321320974613e3bfc5773880545d38ffaa02091a70592120898cedeec0ed37 -->

# asm

The assembler: instructions in, one image and its relocations out.

Port of `qbopt/backend/asm.py`. LLVM's `MCAssembler`: how long each
instruction is, where each therefore lands, which branches can shrink now
that everything is closer, and where each fixup ended up. `select` is the
code emitter above it and `objbuild` the object writer below.

An assembler does not allocate. Handed `assignment=None` this remaps
nothing, which is the right answer for a body whose registers are already
chosen.

Python's `id(op)` is the `Arc` pointer of an LIR occurrence.

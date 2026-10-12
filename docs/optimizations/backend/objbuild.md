> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/objbuild.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/objbuild.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/objbuild.rs sha256:e2e66e79257029910db0adde19f7a5cfd3f754ca3362a22d035a64bf67c31c3c -->

# objbuild

A masm module as an object: its code and data laid out in sections, each
name resolved to where it is defined or declared, each reference to one a
relocation. The result is a format-neutral `llrm_object::Object`; a writer
crate turns it into bytes. A reference to anything the module defines is a
relocation against its section with the addend in the code, as jwasm writes
it; anything else is an undefined symbol.

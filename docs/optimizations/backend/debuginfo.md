> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/debuginfo.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/debuginfo.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/debuginfo.rs sha256:968494cf5f87f18b07a657e25cfe0f1178fbf4162b7ad30cef4991ce2fdecd68 -->

# debuginfo

`-g`: a module's debug information as `llrm_object::debug::Info`,
whichever format a writer encodes it in. `described` reads MIR's metadata
(`llrm_mir::debuginfo`); once the code is laid out, `laid_out` adds each
procedure's place, variables and lines.

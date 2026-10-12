> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/debuginfo.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/debuginfo.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/debuginfo.rs sha256:35da73da424f2a59fd3d8790fe1e164a91b712a198bf0ebc7e9cef97d48b1ade -->

# debuginfo

`-g`: a module's debug information as `llrm_object::debug::Info`,
whichever format a writer encodes it in. `described` reads MIR's metadata
(`llrm_mir::debuginfo`); once the code is laid out, `laid_out` adds each
procedure's place, variables and lines.

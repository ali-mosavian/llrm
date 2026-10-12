> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/registerinfo.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/registerinfo.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/registerinfo.rs sha256:97797cd7b86181572954bcf2c3aba13b20d1ab617a1c6fd2b1a2d1b22585638c -->

# registerinfo

A target's register file, as its description states it
(`registers.regs`) and the target generated it: the `Info` a `Target` hands
out and every query of a register asks. A `RegId` is iced's `Register`
until the newtype (row 11).

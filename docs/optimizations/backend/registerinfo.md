> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/registerinfo.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/registerinfo.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/registerinfo.rs sha256:7cf73bd35b097ba90a4998620adf82d42df253b63f8c2ee10eb76b2c0284f4f5 -->

# registerinfo

A target's register file, as its description states it
(`registers.regs`) and the target generated it: the `Info` a `Target` hands
out and every query of a register asks. A `RegId` is iced's `Register`
until the newtype (row 11).

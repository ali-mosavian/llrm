> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/fixednarrow.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/fixednarrow.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/fixednarrow.rs sha256:94e16c92c6f779a3a9ce9dc78e762cc349b115531b4e2a127f2c751ba6ce6c52 -->

# fixednarrow

A fixed-point product whose operands' product fits the width:
`llvm.smul.fix(a, b, s)` is `(a * b) >> s` at the width, as `mul` and `ashr`
are, where `ranges` proves the 64-bit product the intrinsic stands for never
exceeds 32 bits. isel lowers the intrinsic to a one-operand `imul` and
`shrd`, which take EAX and EDX; the narrow form is `imul r, r` and `sar`.

The test is on the operands: `smul.fix(65536, 65536, 16)` is 65536, but its
product is 2^32.

A factor `v << k` or `v + v` of the product, which cannot wrap since its
interval fits, leaves it for the scale: `(v * 2^k * b) >> s` is `(v * b) >>
(s - k)`, so `x * 2 * y` in Q8 is one `sar` by 7, and a fixed-point integer
made by `<< s` and scaled back costs nothing. It runs once, before LSR,
which would hide such a shift in an induction variable.

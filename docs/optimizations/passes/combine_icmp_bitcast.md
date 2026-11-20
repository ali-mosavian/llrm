> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_icmp_bitcast.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_icmp_bitcast.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_icmp_bitcast.rs sha256:fc8bde8f2953e86e79e11df129376789a892b60c693b6234d0b1b83884cc7750 -->

# combine_icmp_bitcast

InstCombine's `foldICmpBitCast`, the `sitofp` and `uitofp` arms
([`InstCombineCompares.cpp:3491`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L3491))
at d1106deb71cc.

MIRROR: the order of the arms and each condition.

OWED, in place as `// OWED:` comments: the sign-bit test of a bitcast of an
`fpext`/`fptrunc`, and the `is.fpclass` fold of a special value (no backend
lowers the intrinsic yet).

## How it works
A converted integer is zero exactly when its bits are, and `sitofp` keeps
its sign in the sign bit, so a compare of the bits against zero (or a sign
test) is the compare of the integer.

## Used by
`combine_compares::visit_icmp`.

## Example
```text
%f = uitofp i8 %x to float          %r = icmp eq i8 %x, 0
%b = bitcast float %f to i32
%r = icmp eq i32 %b, 0
```

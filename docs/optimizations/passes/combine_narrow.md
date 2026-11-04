> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_narrow.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_narrow.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_narrow.rs sha256:0a274f8a89665ce56753904c70bc3f7532ceec11de60965fb631c1846e172cde -->

# combine_narrow

`narrowMathIfNoOverflow`: `bo (ext X), (ext Y)` or `bo (ext X), C` as `ext
(bo X, Y)` when the math cannot overflow in the narrow width.

Translation of
[`narrowMathIfNoOverflow`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L2332)
at d1106deb71cc.

MIRROR: the order of the checks, the swap of a subtraction's operands, and
the `nsw` (`sext`) or `nuw` (`zext`) the narrow operation takes.

DEVIATE 1: a wide constant is an integer constant (MIR holds no other
constant that `getLosslessTrunc` could narrow).

## Used by
`visitAdd`, `visitSub` (`TryToNarrowDeduceFlags`) and `visitMul`.

## Example
```text
%a = zext i8 %x to i32                      %n = add nuw i8 %x, 3
%r = add i32 %a, 3                   ==>    %r = zext i8 %n to i32
```
when `%x + 3` cannot wrap in 8 bits.

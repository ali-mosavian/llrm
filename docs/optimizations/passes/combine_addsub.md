> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_addsub.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_addsub.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_addsub.rs sha256:1074bd090730e8b8083a95f606184d7cd9212e78b8936e357f471e59355098a8 -->

# combine_addsub

InstCombine's `visitAdd` and the helpers it calls, from
[`InstCombineAddSub.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAddSub.cpp)
at d1106deb71cc.

`visit_add` is
[`visitAdd`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAddSub.cpp#L1604):
the same steps in the same order, each a function named for the C++ one
with its own permalink. A step that returns `nullptr` there returns `None`
here, one that returns an instruction returns `Some(Visit)` (a new
instruction replaces the visited one; `&I` is `Visit::Changed`).

MIRROR: the order of the steps and each condition.

DEVIATE 1: `foldVectorBinop` is not translated: MIR has no vector
operations.

DEVIATE 2 (temporary, `uselist-port`): steps that call a helper another
family owns are left out and named where they stood:
Each is a `// DEVIATE 2:` comment at its place in the order.
`foldBinopWithPhiOperands`, `foldBinOpIntoSelectOrPhi` and
`foldBinOpSelectBinOp` are the phi and select families'.

DEVIATE 3: `uadd.sat(X, C) + -C --> umin(X, ~C)` is not made: the selector
has no `uadd.sat` lowering. Only the `umax` intrinsic call is matched for
`m_UMax`, not the select-of-compare form.

DEVIATE 4: a constant that `ConstantExpr::getAdd` would leave as an
expression is not built (MIR holds no binary constant expressions): the
fold is skipped where the constants do not fold to a number.

## Used by
`combine::visit`, once the add family's slot is flipped.

## How it works
`visit_add` returns the first step that changes the add. The steps rewrite
it to a cheaper form (`x + x` to `x << 1`, `-a + b` to `b - a`, constants
combined through a `sub` or a `zext`), or replace it by an existing value.
A rewrite that needs new operands builds them with the `Combiner`'s
builder, which folds through `simplify*` first.

## Example
`(x + 1) + ~y` is `x - y`:

```text
%a = add i32 %x, 1
%n = xor i32 %y, -1
%r = add i32 %a, %n   ; %r = sub i32 %x, %y
```

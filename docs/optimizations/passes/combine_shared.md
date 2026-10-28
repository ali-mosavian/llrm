> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_shared.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_shared.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_shared.rs sha256:202c30c739eb062a1061906791facbfbd2ee9ef4a4dd13d8d90ba90f6504d40c -->

# combine_shared

What InstCombine's integer `visit*` functions share from
`InstructionCombining.cpp`: the complexity order that puts constants on the
right, and the reassociation of an associative or commutative operation.

Translations at d1106deb71cc:
[`getComplexity`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Transforms/InstCombine/InstCombiner.h#L166),
[`matchSymmetricPair`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L1320),
[`simplifyAssocCastAssoc`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L415),
[`SimplifyAssociativeOrCommutative`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L500),
`maintainNoSignedWrap` (:365) and `replaceOperand`
(InstCombineInternal.h).

MIRROR: the order of the steps and each step's condition, one for one.

DEVIATE 1: `matchSymmetricPair` matches a `select` pair and a min/max pair
only; the `phi` pair (`matchSymmetricPhiNodesPair`) belongs to the phi
family, whose helpers the `combine_phi` translation owns. Until it lands a
pair of phis is no pair.

MIRROR: `Instruction::isAssociative` is the integer operations and `fadd`
and `fmul` with `reassoc` and `nsz` (`combine_phi::is_associative`); `fadd`
and `fmul` are commutative and ordered by complexity.

DEVIATE 3: a new instruction does not take the name of the one it stands
for (`takeName`): MIR names are not read by any pass.

The functions are called by the `visit*` translations; the dispatch in
`combine::visit` calls them when the family's slot is flipped.

## How it works
`get_complexity` ranks a value (undef 0, constant 1, cast or neg or not 2,
else 3); a commutative operation lists the lower rank on the right.
`simplify_associative_or_commutative` then tries, in LLVM's order, to
reassociate through an inner operation of the same opcode wherever the
operation of two of the three operands simplifies, and to join two inner
operations that each carry a constant. Every step that changes the
operation loops again.

## Used by
`combine_addsub` (`visitAdd`, `visitSub`) and the families after it.

## Example
A constant moves to the right, then `(x + 1) + 2` reassociates to `x + 3`:

```text
%a = add i32 1, %x    ; %a = add i32 %x, 1
%b = add i32 %a, 2    ; %b = add i32 %x, 3
```

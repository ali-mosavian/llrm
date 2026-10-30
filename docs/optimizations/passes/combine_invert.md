> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_invert.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_invert.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_invert.rs sha256:3b03bcc979db7e50ce4661a4eea5790b3e0fa1f9930fd3b5a309448be233b9c8 -->

# combine_invert

InstCombine's `getFreelyInverted` (`getFreelyInvertedImpl` of
InstructionCombining.cpp at d1106deb71cc): the `~V` of a value whose
inversion costs nothing, when every use of it is being rewritten to read
`~V`.

MIRROR: the order of the cases and each condition, for the `not`, an
integer constant, a compare, `add`, `xor`, `sub`, `ashr`, `sext` and
`trunc`.

OWED, in place as `// OWED:` comments: the select and min/max case (it
needs `shouldAvoidAbsorbingNotIntoSelect`), the phi case, and De Morgan's
laws. A value they would invert is not freely invertible here, so a fold
that asks finds less than the pin, never more.

## How it works
`Combiner::get_freely_inverted` with `build` false only asks (it returns
some value when the answer is yes); with `build` it makes the `~V`
before `at`. `consumes` is set when the inversion eats an instruction (a
`not` that is dropped).

## Used by
The equality folds of `combine_icmp_binop.rs` that turn an `and` of a freely
invertible operand into an `or` of its inverse.

## Example
`~(x + 5)` is `-6 - x`:

```text
%x = add i32 %p, 5      ; ~%x = sub i32 -6, %p
```

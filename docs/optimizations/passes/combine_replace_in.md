> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_replace_in.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_replace_in.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_replace_in.rs sha256:9a1fc7262af8ae460026363abf86a720a9da1d9afd62aac357061cc2533a7e21 -->

# combine_replace_in

`replaceInInstruction`: the uses of a value inside the one-use chain that
feeds a `select` arm read a constant instead.

Translation of
[`InstCombinerImpl::replaceInInstruction`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L1658)
at d1106deb71cc, called from `visitSelectInst` (:4644) with the condition
and `true` in the true arm, `false` in the false arm.

MIRROR: two instructions up, each with one use and safe to execute with
its operands replaced.

DEVIATE 1: no lane-crossing check (MIR has no vectors).

## Example
```text
%b = select i1 %c, i8 %q, i8 2          %b = select i1 true, i8 %q, i8 2
%i = icmp ugt i8 %p, %b          ==>    %i = icmp ugt i8 %p, %b
%r = select i1 %c, i1 %i, i1 false      %r = select i1 %c, i1 %i, i1 false
```

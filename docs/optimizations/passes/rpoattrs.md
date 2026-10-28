> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/rpoattrs.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/rpoattrs.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/rpoattrs.rs sha256:4f89c9541110d1a92bfbf63afad02046f06b7b5ffcda86801663b5e56cc668f5 -->

# rpoattrs

LLVM's `ReversePostOrderFunctionAttrsPass`: `norecurse` for an internal
function whose every use is a call from a `norecurse` function.

## How it works
The one-member call-graph components are walked callers first. An internal
defined function with a use is marked `norecurse` when every use is a direct
call of it, and each caller is `norecurse` already: no path from it can come
back without passing a function that does not recurse. A caller marked
earlier in the walk counts.

DEVIATE: the pin also infers `nofpclass` on the parameters and result from
the calls (`addNoFPClassAttrsTopDown`); MIR has no `nofpclass` attribute.

## Example
```text
define void @top() norecurse { call @leaf() }    define internal void @leaf() norecurse
define internal void @leaf() { ... }        ->   (called only from @top)
```

## Runs
After `available-externally` in the module pipeline
(PassBuilderPipelines.cpp:1542).

## See also
`deduceFunctionAttributeInRPO`
([FunctionAttrs.cpp:2394](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/IPO/FunctionAttrs.cpp#L2394)),
`addNoRecurseAttrsTopDown`
([:2323](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/IPO/FunctionAttrs.cpp#L2323)).

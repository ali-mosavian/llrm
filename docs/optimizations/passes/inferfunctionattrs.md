> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/inferfunctionattrs.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/inferfunctionattrs.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/inferfunctionattrs.rs sha256:85a2c582425eca28650e8095386281e1a7b1670e6c6734b745a2289456de5e6c -->

# inferfunctionattrs

A library function the module only declares (`sin`, `strlen`, `memcmp`)
gets the attributes the C library promises of it, as LLVM's
`InferFunctionAttrsPass`.

## How it works
A translation of LLVM `d1106deb71cc`
[`InferFunctionAttrs.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/IPO/InferFunctionAttrs.cpp)
and `inferNonMandatoryLibFuncAttrs` of `BuildLibCalls.cpp`. A declaration
whose name and prototype are a `LibFunc` the target's runtime has
(`TargetLibraryInfo`) takes the attributes of its case in
`libfunc/attrs.rs` (generated from the pin by `tools/gen/gen_libattrs.py`):
`nounwind`, `willreturn`, `nofree`, `nosync`, `nocallback`, what it does to
memory (`memory(errnomem: write)` for `sin`: it sets `errno` and touches
nothing else) and what each pointer parameter is. A call of `sin(x)` twice
is then one call to the optimizer's CSE.

## Example
```text
  declare double @sin(double)
  ==>
  declare double @sin(double) nofree nosync nounwind willreturn nocallback memory(errnomem: write)
```

DEVIATE (anything MIR or Rust lacks):
1. `allocsize` and `allockind` keep their name only (the attribute list
   parser drops their arguments), `allocalign` and `allocptr` are not kept,
   and `alloc-family` is a string attribute: no pass here reads them.
2. `-fno-builtin` and `optnone` are the function's own flags: a declaration
   that has `optnone` is left alone, as the pin's.
3. `getRtLibUseGOT` (`nonlazybind`) is no module flag here.
4. `isLibFreeFunction` is whether the case set `allockind(free)`.

## Runs
Once, at the start of the module pipeline
(`PassBuilderPipelines.cpp#L1169`).

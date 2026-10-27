> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_build.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_build.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_build.rs sha256:fd18028e76831f8c715aa3e421f391a843b9b5c0bf11e3bd62f91953ca1b03cd -->

# combine_build

`InstCombiner::Builder`: the `IRBuilder` over `InstSimplifyFolder` that
InstCombine's `visit*` functions build new instructions with.

Translations at d1106deb71cc:
[`InstSimplifyFolder`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/InstSimplifyFolder.h#L25),
whose `Fold*` return `simplify*` of the operands or nothing, and the
`Create*` of
[`IRBuilder`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/IRBuilder.h)
that ask the folder first and otherwise insert (`InsertNewInstWith`, which
queues the instruction on the worklist, is `Combiner::insert_before`).

MIRROR: a `create_*` returns the value `simplify*` finds, else a new
instruction before `at`. The folder's query has no dominator tree, no
assumptions and no context instruction (`InstSimplifyFolder(DL)`), as here.

DEVIATE 1: no instruction names (`Twine`): MIR names are not read by any
pass.

DEVIATE 2: `CreateIntrinsic` (`Combiner::create_intrinsic`) builds a call
to the declaration the pass makes through `Unit::declared`
(`Intrinsic::getOrInsertDeclaration`); the declaration reaches the module
after the pass. It is for intrinsics overloaded on one integer type, named
`llvm.<base>.i<width>`, with the argument types as the parameters.

## How it works
Each `create_*` asks `simplify*` of its operands first (the folder) and
inserts a new instruction only when nothing simpler exists; the instruction
is queued on the worklist. `replace_with_new` is what `visit*` returns for
a `BinaryOperator::Create`: insert, replace the visited instruction, erase.

## Used by
`combine_addsub` and the families after it.

## Example
`create_binop(at, Add, x, 0)` is `x`, found by `simplifyAddInst`;
`create_binop(at, Add, x, y)` is a new `add` before `at`.

# Plan: the fold work split

Two owners, no shared file. Status: proposed, awaiting oversight.

## Layers (one owner per fact)

LLVM layers the folds; llrm keeps the layers.

| llrm file | LLVM source | owner | role |
|---|---|---|---|
| `constfold.rs` | [`ConstantFold.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFold.cpp) | fold-poc | folds an operation of constants (`binary`, `cast`, `compare`, `select`); called by `build::Builder` and `simplify.rs` |
| `constantfolding.rs` (new) | [`ConstantFolding.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp) | valuetracking-port | folds an instruction whose operands are constants, with the data layout; its binary, cast, compare and select cases call `constfold.rs` |
| `simplify.rs` | [`InstructionSimplify.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/InstructionSimplify.cpp) | fold-poc | the value an instruction equals |

`constfold.rs` is not replaced: `ConstantFolding.cpp` calls `ConstantFoldBinaryInstruction` and its siblings, which are `constfold.rs`. The two cover disjoint functions and nothing moves or is deleted.

## valuetracking-port (new files only)

1. `constantfolding.rs`: integer and pointer arms of `ConstantFolding.cpp` (`ConstantFoldInstruction`, `ConstantFoldInstOperands`, `ConstantFoldCompareInstOperands`, `ConstantFoldCastOperand`, GEP and `ptrtoint`, `ConstantFoldLoadFromConst`, `ReadDataFromGlobal`, integer intrinsic folds).
2. `simplify_calls.rs`: `simplifyIntrinsic`, `simplifyUnaryIntrinsic`, `simplifyBinaryIntrinsic`, `simplifyCall`, `simplifyICmpWithIntrinsicOnLHS`.
3. `simplify_select.rs`: `simplifySelectWithICmpCond`, `simplifySelectWithBitTest`, `simplifySelectWithEquivalence`, `simplifyICmpWithDominatingAssume`.

Each function names its C++ permalink. Tests: LLVM's `test/Transforms/InstSimplify` inputs compared with the pinned LLVM, fail first, with the premise rule (an oracle that can only say one thing proves nothing).

Agreed with fold-poc (in writing, to the coordinator):

- `constantfolding.rs` calls `constfold.rs` for the binary, cast, compare and select folds and does not translate them again.
- `constant_fold_load_from_const(context, layout, c: ConstantId, ty, offset: u64) -> Option<ConstantId>` and `constant_fold_load_from_const_ptr(context, layout, globals, ptr: ConstantId, ty) -> Option<ConstantId>` keep the signatures fold-poc gave gvn-port (#2160).
- `simplify_select.rs` and `simplify_calls.rs` recurse into `simplify.rs`, whose `Query` is private. fold-poc moves it to its own `pub(crate)` file in a small PR with no behaviour change, right after the ValueTracking port; until then they are drafted against `ValueQuery`, `&mut Context` and `max_recurse: u32`.

## fold-poc

`simplify.rs`, `constfold.rs`, the dispatch (one match arm per entry point above), pass wiring (combine, EarlyCSE, GVN, fold), the ValueTracking consumer switch, the `Query` move, and the functions `simplify.rs`'s header lists as owed and this plan does not (`AndOrOfCmps`, `AndOrWithICmpEq`, `threadBinOpOverSelect`/`PHI`) after the wiring PR.

## Interface

The new entry points take `simplify.rs`'s `Query` shape and return `Option<Operand>` or `Option<ConstantId>`; fold-poc reviews the signatures before bodies are written.

## Order

ValueTracking port, then fold-poc's consumer, then 1, 3, 2; one PR each.

## Not in the plan

Floating-point folds (no reader), vectors, `undef`.

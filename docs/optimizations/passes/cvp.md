> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/cvp.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/cvp.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/cvp.rs sha256:607ef2c9cfdd11e2901841edb4a39e481dc07a6008663e4f8ee4478fec90fae3 -->

# cvp

Correlated value propagation: a value is replaced, narrowed or given a flag
when the branches and ranges around it prove the fact, as LLVM's
`correlated-propagation`.

## How it works
A translation of LLVM `d1106deb71cc` `CorrelatedValuePropagation.cpp`, one
Rust function per C++ function in file order, each linking its line. The
pass walks the blocks reachable from the entry in depth-first preorder and
asks [`llrm_mir::lazyvalueinfo::LazyValueInfo`](https://github.com/ali-mosavian/llrm/blob/main/crates/ir/llrm-mir/src/lazyvalueinfo.rs) (the translation of
`LazyValueInfo.cpp`) for the constant, range or predicate of an operand at
its use. `runImpl` visits each instruction once, then the terminator, and
gives the function a `range` return attribute when every `ret` is bounded.
The analysis is kept across edits and told of every instruction erased
(`forget_value`), as LLVM's value handles do, so a function of N
instructions costs O(N) solves (`steps` counts them; the slope test in
`cvp_tests.rs` doubles a function and checks the count does not more than
double).

Beside the pass, the C++ it calls that MIR does not hold is translated
here with its permalink: `BasicBlock::removePredecessor`,
`PHINode::removeIncomingValue`, `PHINode::hasConstantValue`, the `switch`
arm of `ConstantFoldTerminator`, `SwitchInst::removeCase`,
`SwitchInstProfUpdateWrapper`, `DominatorTree::dominates(Instruction,
BasicBlock)` and `GlobalValue::hasExactDefinition`.

DEVIATE (anything MIR or Rust lacks):
1. No vectors: MIR has no vector operations, so each `process*` function
   takes a scalar integer only (`getValueOnEdge`'s vector-condition test and
   `processSelect`'s `isVectorTy` are the scalar case).
2. No `undef`; `poison` stands for it where LVI hands back an `UndefValue`.
3. No operand bundles: the `deopt` loop of `processCallSite` (lines 694-713)
   has nothing to read.
4. `fcmp`: LVI's `getPredicateAt` takes an `ICmpInst` predicate and holds no
   floating-point fact (`LazyValueInfo.cpp` has no `FCmp`), so `processCmp`
   leaves an `fcmp` alone; only a compare of two constants, which
   `instsimplify` folds, differs.
5. `simplifyInstruction(P, SQ)` of `processPHI` is
   [`llrm_mir::simplify::simplify_instruction`](https://github.com/ali-mosavian/llrm/blob/main/crates/ir/llrm-mir/src/simplify.rs), MIR's integer subset of
   `InstructionSimplify.cpp` without the dominator tree or the assumptions
   the C++ query holds.
6. `ConstantFoldTerminator` is its `switch` arm alone (the terminator
   `processSwitch` passes is a `switch`), with no `DeleteDeadConditions` (it
   is false there). `DomTreeUpdater` is the dominator tree brought up to
   date from the function's change log before its next query
   (`Analyses::get`), the lazy strategy's effect.
7. The `umin`/`umax` made by `processMinMaxIntrinsic` is declared through
   `Declared`, which the module has after the pass; LVI is given the same
   declaration beside the module's (`function_declaration`) while the run
   lasts, unless an earlier pass of the pipeline already declared a name
   (its id is then past the module's), when LVI answers the full range
   through the call.
8. `CreateSelectWithUnknownProfile` adds `!prof` only when the function has
   profile data; MIR has none.
9. `STATISTIC` counters and `takeName` are not kept; debug locations do not
   exist.
10. `Function::hasExactDefinition` ignores `getSemanticInterposition` and
    `isDSOLocal` (MIR has no module flags or visibility); a function the
    pass is run on without its id in the module (`Unit::id` is `None`) has
    no `range` attribute inferred.
11. `BasicBlockEdge` dominance of an `invoke` result counts the predecessors
    of the normal destination once each (`Function::predecessors` lists a
    block once); two edges from one `invoke` to one block cannot occur.
12. `SwitchInst::removeCase` keeps LLVM's order (the last case takes the
    removed one's place); `LVI` is not told of the `switch` edges removed,
    as in the C++.

## Example
In the block entered when `%x u< 10`, `udiv i32 %x, 16` is `0`: the range
`[0, 10)` is below the divisor.

## Runs
Alone, through `llrm-pass cvp`; not in the pipeline. `tools/difftest`
compares it with `opt -passes=correlated-propagation`.

## Keeps
Dominators and loops where no `switch` changed an edge.

## See also
[`llrm_mir::lazyvalueinfo`](https://github.com/ali-mosavian/llrm/blob/main/crates/ir/llrm-mir/src/lazyvalueinfo.rs), [`llrm_mir::constantrange`](https://github.com/ali-mosavian/llrm/blob/main/crates/ir/llrm-mir/src/constantrange.rs).

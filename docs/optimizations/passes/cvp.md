> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/cvp.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/cvp.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/cvp.rs sha256:f1414d653d31a985b3c4f31885d22d183b050c8f5335d87d4cf31210a3faa6ec -->

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
`SwitchInstProfUpdateWrapper`, `DomTree::dominates(Instruction,
BasicBlock)` and `GlobalValue::hasExactDefinition`.

DEVIATE, permanent (MIR or Rust lacks the construct):
1. No vectors: MIR types vectors but has no lane operation and nothing
   produces one, so each `process*` function takes a scalar integer only
   (#2201).
2. No `undef`; `poison` stands for it where LVI hands back an `UndefValue`
   (#2202).
3. No operand bundles: the `deopt` loop of `processCallSite` (lines 694-713)
   has nothing to read (#2203).
4. `DomTreeUpdater` is the dominator tree brought up to date from the
   function's change log before its next query (`Analyses::get`), the lazy
   strategy's effect; `ConstantFoldTerminator` is its `switch` arm alone
   (the terminator `processSwitch` passes is a `switch`), with no
   `DeleteDeadConditions` (it is false there).
5. `CreateSelectWithUnknownProfile` adds `!prof` only when the function has
   profile data; MIR has none. `STATISTIC` counters and `takeName` are not
   kept; debug locations do not exist.
6. `Function::hasExactDefinition` ignores `getSemanticInterposition` and
   `isDSOLocal` (MIR has no module flags or visibility); a function the pass
   is run on without its id in the module (`Unit::id` is `None`) has no
   `range` attribute inferred.
7. `BasicBlockEdge` dominance of an `invoke` result counts the predecessors
   of the normal destination once each (`Function::predecessors` lists a
   block once); two edges from one `invoke` to one block cannot occur.

OWED (MIR can represent it; each has an issue and its exit):
8. `fcmp`: LVI's `getPredicateAt` takes an `ICmpInst` predicate and holds no
   floating-point fact, so `processCmp` leaves an `fcmp` alone; only a
   compare of two constants, which `instsimplify` folds, differs. Exit:
   floating-point constant folding (#2246).
9. `simplifyInstruction(P, SQ)` of `processPHI` is
   [`llrm_mir::simplify::simplify_instruction`](https://github.com/ali-mosavian/llrm/blob/main/crates/ir/llrm-mir/src/simplify.rs), MIR's integer subset of
   `InstructionSimplify.cpp` without the dominator tree or the assumptions
   the C++ query holds. Exit: `simplify_instruction_in` once #2245 is in
   main.
10. The `umin`/`umax` made by `processMinMaxIntrinsic` is declared through
    `Declared`, which the module has after the pass; LVI is given the same
    declaration beside the module's (`function_declaration`) while the run
    lasts, unless an earlier pass of the pipeline already declared a name,
    when LVI answers the full range through the call: #2347.
11. MIR gives no `speculatable` to the saturating intrinsics and no
    `range(-1,2)` to `scmp`/`ucmp` (LVI note 13): eight functions of LLVM's
    tests differ. Exit: #2142.
12. Not linear: `Function::insert(Before)` and `erase` scan the block, so
    one block with many conversions is quadratic (20,000 `sext`: 0.28 s, 4x
    per doubling). The pass's own work is O(n) (slope test on LVI solves).
    Exit: #2348.

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

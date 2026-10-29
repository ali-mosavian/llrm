---
architecture-index: LLVM's SimplifyCFG against llrm's `decide`
architecture-order: 370
---
# LLVM's SimplifyCFG against llrm's `decide`

TL;DR: SimplifyCFG folds a branch whose condition is a constant it can see in the block and asks no analysis; `decide` first builds the whole function's facts, ranges and guards and then looks at 574 branches to decide none of them on d_faces `-O2`: 151 M (4.7%) against SimplifyCFG's 8.4 M (2.0%).

## LLVM

`simplifyOnce` works one block at a time: it removes an unreachable block, calls `ConstantFoldTerminator` (a branch on a constant, a switch on a constant) and then merges, hoists and sinks; the facts it uses are in the block and its operands [`llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9303-L9324`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9303-L9324):

```cpp
bool SimplifyCFGOpt::simplifyOnce(BasicBlock *BB) {
  bool Changed = false;

  assert(BB && BB->getParent() && "Block not embedded in function!");
  assert(BB->getTerminator() && "Degenerate basic block encountered!");

  // Remove basic blocks that have no predecessors (except the entry block)...
  // or that just have themself as a predecessor.  These are unreachable.
  if ((pred_empty(BB) && BB != &BB->getParent()->getEntryBlock()) ||
      BB->getSinglePredecessor() == BB) {
    LLVM_DEBUG(dbgs() << "Removing BB: \n" << *BB);
    DeleteDeadBlock(BB, DTU);
    return true;
  }

  // Check to see if we can constant propagate this terminator instruction
  // away...
  Changed |= ConstantFoldTerminator(BB, /*DeleteDeadConditions=*/true,
                                    /*TLI=*/nullptr, DTU);

  // Check for and eliminate duplicate PHI nodes in this block.
  Changed |= EliminateDuplicatePHINodes(BB);
```

## llrm

`_decisions` visits every conditional terminator of the function with facts computed beforehand for all of it: the registers' and memory facts (`ThroughMemory`), the constant cycles (`constant_cycles::propagated`), the counted loops' ranges (`ranges::bounds`) and the guards. A branch is decided by the first stage that settles it: facts, then ranges on the edges, then guards [`crates/opt/llrm-transforms/src/decide.rs#L229-L261`](https://github.com/ali-mosavian/llrm/blob/21f5c9fc8e67b5bf405cf9ecdaf65247a26c0450/crates/opt/llrm-transforms/src/decide.rs#L229-L261):

```rust
    let scoped = ranges::bounds(unit)?;
    let mut out = Vec::new();
    for &block in function.layout() {
        let Some(last) = function.terminator(block) else {
            continue;
        };
        let instruction = function.instruction(last);
        if instruction.opcode == Opcode::Switch {
            out.extend(_switch_target(unit, last, &facts).map(|target| (block, target)));
            continue;
        }
        let [_, Operand::Block(taken), Operand::Block(other)] = instruction.operands[..] else {
            continue;
        };
        let mut answer = _outcome(unit, block, &facts, Some(&nonnull));
        if answer.is_none() {
            let scope = scoped.scope(unit, cfg::id(block));
            let mut possible = Vec::new();
            for target in function.successors(block) {
                if ranges::on_edge_over(unit, block, target, &scope, Some(&facts))?.is_some() {
                    possible.push(target);
                }
            }
            if let [one] = possible[..] {
                answer = Some(one == taken);
            }
        }
        if answer.is_none() {
            answer = _implied(unit, block, last);
        }
        out.extend(answer.map(|answer| (block, if answer { taken } else { other })));
    }
    Ok(out)
```

## Divergence

LLVM's rule is the cheap syntactic one and the rest is left to other passes (CorrelatedValuePropagation 6.7 M, JumpThreading 4.5 M); `decide` builds the inputs of all three for the whole function first: `Held::of` 81.5 M, `loop_bounds` 74.0 M and `_decisions` 78.1 M (65.3 M of it `propagated`), shared with other passes that ask the same pieces. It decides little: over 421 compiles (the 66 vsgcc programs, the `tests/run` fixtures and QCport modules, `-O2` and `-Os`) 245 of about 22,000 branch visits, 126 of them a branch on a constant condition and none on a loaded value, and 387 of the 421 compiles decide nothing and carry 72.7% of its cost. Building the memory facts only for a condition that holds a load, and the cycles only for one that holds a phi, added 0.6% to decide and 0.03% to the compile, because `fold` asks the facts first and almost every condition holds a phi or a load.

## Measured

| | LLVM (clang 24, cc1) | llrm |
|---|---:|---:|
| pass, inclusive | `SimplifyCFGPass::run` 8.37 M (2.02%), 88 runs; `simplifyCFG` 6.18 M | `Decide::run` 151.2 M (4.69%), 35 runs |
| inputs built first | none | `Held::of` 81.5 M, `loop_bounds` 74.0 M, `propagated` 65.3 M (overlap with other passes' asks) |
| branches decided, d_faces | n/a | 0 of 574 visits at `-m32 -march=i486 -fno-inline-functions`; 9 with the corpus flags |
| decided, 421 compiles | n/a | 245 of about 22,000 visits (facts 142, ranges 52, implied 11, switches 40) |

Input `render/d_faces.c`, `-m32 -march=i486 -fno-inline-functions -O2` unless stated. LLVM: callgrind on `clang -cc1` of d1106deb71cc, total 415.4 M. llrm: callgrind on the release `llrm-cc` of 21f5c9fc8, total 3,224 M; decision counts from a counter at each conditional terminator of `_decisions`.

## Proposed change

- **Files**: `crates/opt/llrm-transforms/src/decide.rs` (`_decided`, `_decisions`); the shared inputs `Held::of`, `loop_bounds` and `constant_cycles::propagated` in `crates/opt/llrm-analysis`.
- **What**: build the memory facts only for a condition that holds a load and the cycles only for one that holds a phi.
- **Expected saving**: none. On 531 compiles decide's cost rose 0.6% and the compile 0.03%: `fold` asks the facts first, and almost every condition holds a phi or a load. No load-derived fact decided a branch in the corpus (245 of about 22,000 visits decided). What remains of decide's 151 M at `d_faces` `-O2` sits in the shared tower (`Held::of` 81.5 M, `loop_bounds` 74.0 M), which keep-analyses is flattening.
- **Status**: measured and not kept (branch `notkept/decide-lazy-memory`; [compile-time-spec section 8](../../architecture/compile-time-spec.md), "Decide's facts only where a branch needs them").

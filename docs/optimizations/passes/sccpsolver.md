> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/sccpsolver.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/sccpsolver.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/sccpsolver.rs sha256:20d3a366b6015d49de6c828f3425dafc7dbff618ca185517e7ac20a115081e89 -->

# sccpsolver

Sparse conditional constant propagation: the solver. A translation of the
parts of LLVM's `SCCPSolver.cpp` (d1106deb71cc) a function pass reaches:
an optimistic data flow over executable blocks and feasible edges, a
`ValueLatticeElement` per value, and the rewrites the result allows.

## How it works
`Solver::solve` takes a block off the block work list, visits its
instructions once, and puts the users of every value whose lattice moved on
the instruction work list; only executable blocks and feasible edges are
visited. A value starts unknown, becomes undef, a constant, a range, or
overdefined, and only moves towards overdefined. When the lists are empty,
`Solver::resolved_undefs_in` makes what is still unknown overdefined (an
`undef` operand can be any value) and the solve resumes until nothing is
left to resolve. [`crate::sccp`](sccp.md) then rewrites:
`Solver::simplify_insts_in_block` replaces values with their constants,
turns signed operations that cannot be negative into unsigned ones, adds
`nuw`/`nsw`/`nneg` the ranges prove, and drops range checks;
`Solver::remove_non_feasible_edges` cuts the edges that were never taken.

```text
 b0: br i1 true, b1, b2     b0: br b1      (b2 never executes)
 b1: %x = add i32 1, 2      b1: ret i32 3
```

## DEVIATEs
1. No PredicateInfo: `addPredicateInfo`, `handlePredicate` and
   `removeSSACopies` are absent, so the facts a branch condition gives its
   targets (`ssa.copy`) are not used. Fact: llrm has no PredicateInfo
   translation. Exit: PredicateInfo translation (scalar-cleanup-port, next).
2. Nothing interprocedural: `TrackedGlobals`, the argument-tracking set,
   `MustPreserveReturnsInFunctions`, the specialisation entry points and
   `MRVFunctionsTracked` are not translated; `runSCCP` never fills them. The
   one tracked return value is the function's own (`TrackedRetVals`), for a
   call to itself and for `inferReturnAttributes`.
3. No vectors and no scalable vectors: the `vscale` and
   `experimental.get.vector.length` arms of `handleCallResult` are absent.
4. A floating-point compare is not folded: `ConstantFoldCompareInstOperands`
   in MIR takes integer predicates (an `fcmp` goes overdefined once its
   operands are known). Exit: the ConstantFolding translation's FP fold.
5. No `DomTreeUpdater`: the manager recomputes dominators when the CFG
   changed.
6. No `!implicit.ref`, `clang.arc.attachedcall` bundles or `indirectbr`: MIR
   has none.
7. A phi has one input per edge in MIR, so a block reached twice by one
   predecessor is read twice (`removePredecessor` per edge, as ADCE).

## Example
`%c = icmp eq i32 %a, %a` with `%a` unknown is overdefined; with `%a = 1` it
is `true`, and the branch on it goes to one successor.

## See also
- Uses: [`llrm_mir::valuelattice`](https://github.com/ali-mosavian/llrm/blob/main/crates/ir/llrm-mir/src/valuelattice.rs), [`llrm_mir::constantrange`](https://github.com/ali-mosavian/llrm/blob/main/crates/ir/llrm-mir/src/constantrange.rs),
  [`llrm_mir::constantfolding`](https://github.com/ali-mosavian/llrm/blob/main/crates/ir/llrm-mir/src/constantfolding.rs), [`llrm_mir::simplify`](https://github.com/ali-mosavian/llrm/blob/main/crates/ir/llrm-mir/src/simplify.rs)
- Used by: [`crate::sccp`](sccp.md)

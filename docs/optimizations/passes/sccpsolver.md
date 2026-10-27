> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/sccpsolver.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/sccpsolver.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/sccpsolver.rs sha256:2917c3b5bbb0158074ccdb00a28663adc1201412ea2f12649c5140be67ccde5a -->

# sccpsolver

Sparse conditional constant propagation: the solver. A translation of
LLVM's `SCCPSolver.cpp` (d1106deb71cc) without function specialisation:
an optimistic data flow over executable blocks and feasible edges, a
`ValueLatticeElement` per value, and the rewrites the result allows. It
solves one function (`Solver::new`, for `runSCCP`) or a module
(`Solver::module`, for `runIPSCCP`).

## How it works
`Solver::solve` takes a block off the block work list, visits its
instructions once, and puts the users of every value whose lattice moved on
the instruction work list; only executable blocks and feasible edges are
visited. A value starts unknown, becomes undef, a constant, a range, or
overdefined, and only moves towards overdefined. When the lists are empty,
`Solver::resolved_undefs_in` makes what is still unknown overdefined (an
`undef` operand can be any value) and the solve resumes until nothing is
left to resolve.

Across a module the work lists are shared and each entry names its
function. A call to a function whose arguments are tracked makes its entry
executable and joins the actuals into its arguments; a tracked return value
(`TrackedRetVals`, or one per field of a struct, `MRVFunctionsTracked`) is
joined from each `ret` and flows to each call; a tracked global
(`TrackedGlobals`) is joined from each store and read by each load. The
caller chooses what is tracked. [`crate::sccp`](sccp.md) then rewrites:
`Solver::simplify_insts_in_block` replaces values with their constants,
turns signed operations that cannot be negative into unsigned ones, adds
`nuw`/`nsw`/`nneg` the ranges prove, and drops range checks;
`Solver::remove_non_feasible_edges` cuts the edges that were never taken.

```text
 b0: br i1 true, b1, b2     b0: br b1      (b2 never executes)
 b1: %x = add i32 1, 2      b1: ret i32 3
```

## DEVIATEs
1. `PredicateInfo` has no operand-bundle assumes ([`crate::predicateinfo`](predicateinfo.md)
   DEVIATE 1); `removeSSACopies` is in [`crate::sccp`](sccp.md).
2. Interprocedural: translated are
   [`addTrackedFunction`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp#L919),
   [`trackValueOfGlobalVariable`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp#L911),
   [`addArgumentTrackedFunction`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp#L937)
   and its queries,
   [`addToMustPreserveReturnsInFunctions`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp#L929),
   [`markFunctionUnreachable`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp#L1044),
   [`solveWhileResolvedUndefsIn(Module&)`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp#L1049),
   and the tracked arms of
   [`visitReturnInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp#L1489),
   [`visitStoreInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp#L1902),
   [`visitLoadInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp#L1945),
   [`handleCallArguments`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp#L2047),
   [`handleCallResult`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp#L2152)
   and
   [`resolvedUndef`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp#L2286).
   Not translated: the specialisation entry points (`invalidate`,
   `resetLatticeValueFor`, `setLatticeValueForSpecializationArguments`,
   `solveWhileResolvedUndefs`, `Invalidated`), which only
   `FunctionSpecializer` calls. Exit: its translation.
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
8. `ValueId` and `InstId` are local to a function, so the state keyed by
   value is per function: the visited function's state is in the solver's
   fields and the others are parked. A work-list entry names its function.
9.
   [`pushUsersToWorkList`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp#L1105)
   of a function or a global: MIR has no use lists of constants, so the
   users pushed are the calls and loads that read the tracked value, in the
   order they first did. The pin pushes only visited instructions too, and a
   store revisited changes nothing; the order of the pushes differs.
   `resolvedUndefsIn(M)` runs over the functions given to
   `Solver::module`, in that order; one not given has nothing executable.

## Example
`%c = icmp eq i32 %a, %a` with `%a` unknown is overdefined; with `%a = 1` it
is `true`, and the branch on it goes to one successor.

## See also
- Uses: [`llrm_mir::valuelattice`](https://github.com/ali-mosavian/llrm/blob/main/crates/ir/llrm-mir/src/valuelattice.rs), [`llrm_mir::constantrange`](https://github.com/ali-mosavian/llrm/blob/main/crates/ir/llrm-mir/src/constantrange.rs),
  [`llrm_mir::constantfolding`](https://github.com/ali-mosavian/llrm/blob/main/crates/ir/llrm-mir/src/constantfolding.rs), [`llrm_mir::simplify`](https://github.com/ali-mosavian/llrm/blob/main/crates/ir/llrm-mir/src/simplify.rs)
- Used by: [`crate::sccp`](sccp.md)

## Used by
Named by `ipsccp`, `sccp`.

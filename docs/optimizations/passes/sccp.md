> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/sccp.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/sccp.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/sccp.rs sha256:c669ab5f7518e766e3858d494cfb58ccb11876f9c7223ce25b0741867a726b9a -->

# sccp

Sparse conditional constant propagation. A translation of LLVM's `SCCP.cpp`
(`runSCCP`) at d1106deb71cc with the rewrites of `SCCPSolver.cpp` it calls:
solve for constants over the executable part of the function, replace each
value with its constant, empty the blocks that never run, and cut the
edges that were never taken.

## How it works
[`crate::sccpsolver::Solver`](sccpsolver.md) marks the entry block executable, gives each
argument the state its attributes say, and runs to a fixed point; what is
left unknown is made overdefined and the solve resumes until nothing
changes. Then, per block: an unreachable block is emptied (a phi stays; the
rest becomes `unreachable`) and deleted last; a reachable one has each
value replaced by its constant, a signed operation on non-negative values
made unsigned, wrap flags added that the ranges prove, and a few
range-based simplifications. A branch or switch with an edge that was never
feasible loses it. The function's return range is added to its `range`
attribute.

The pass is on LLVM's pipeline as `ipsccp` then `sccp`; here it is the
function-level one at `sccp@792`.

```text
 b0: %c = icmp eq i32 1, 1      b0: br label %b1
     br i1 %c, b1, b2           b1: ret i32 7
 b1: ret i32 7                  (b2 deleted)
 b2: ret i32 8
```

DEVIATEs: those of [`crate::sccpsolver`](sccpsolver.md).

## Example
See `sccp_tests.rs`.

## See also
- Uses: [`crate::sccpsolver`](sccpsolver.md), [`crate::local::change_to_unreachable`](local.md)

## Used by
Named by `calledvaluepropagation`, `ipsccp`, `pipeline`.

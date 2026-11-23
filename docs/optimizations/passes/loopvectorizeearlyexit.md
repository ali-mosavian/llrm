> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopvectorizeearlyexit.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopvectorizeearlyexit.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopvectorizeearlyexit.rs sha256:56537c4244aa8eeb4525ec44993de86b97a4544835ed8324677e4535f3d3a7be -->

# loopvectorizeearlyexit

Early-exit loops: the plan of several blocks and the control flow around it
(`handleUncountableEarlyExits`, `createVectorLoopSkeleton`).

## How it works
A loop that leaves where a computed condition says so, read-only and with
loads dereferenceable for every trip (legality:
`isVectorizableEarlyExitLoop`), is widened into a vector loop of its own and
the original loop stays as the scalar remainder. The vector loop runs `VF`
iterations a trip; its exit is the pin's `BranchOnTwoConds` of the any-of of
the exit condition and the count (`expandBranchOnTwoConds` makes the two
branches). IC is 1 (`selectInterleaveCount`: `Plan.hasEarlyExit()`).

```text
preheader:                            vector.ph:
  %min.iters.check = icmp ult n, VF     %n.vec = n - n % VF
  br %min, scalar.ph, vector.ph         br vector.body
vector.body:
  %index = phi [start, vector.ph], [%index.next, vector.body.interim]
  wide loads, compare:      %mask = icmp ne <VF x i8> ..
  %index.next = add %index, VF
  %any = call @llvm.vector.reduce.or(%mask)      (AnyOf)
  br %any, vector.early.exit, vector.body.interim
vector.body.interim:   br (%index.next == %ind.end), middle.block, vector.body
vector.early.exit:     the exit phis' values at the first set lane, br early exit
middle.block:          br (n == n.vec), countable exit, scalar.ph
scalar.ph:             %bc.resume.val = phi [%ind.end, middle.block], [start, preheader]
body (the original):   from %bc.resume.val
```

## DEVIATE rows
- VPEE1 one uncountable exit, and the countable one is the latch's (the pin
  chains `vector.early.exit.check` blocks for several). Exit: the chain.
- VPEE2 the recipes are loads of consecutive addresses, integer operations
  on loaded and loop-invariant values, and one integer compare as the exit
  condition (`i1` operations and selects are left scalar).
- VPEE3 `FirstActiveLane` is the `reduce.umin` of the lane numbers the mask
  selects, not `llvm.experimental.cttz.elts` (llrm has no such intrinsic,
  and its verifier takes no `<N x i1>` bitcast to an integer).
- VPEE4 the trip count is `bound - start` (LVS1 of the skeleton): an
  induction stepping by one against an `eq`/`ne` test of the stepped value.
- VPEE5 the cost of the any-of reduction (`VPInstruction::AnyOf`, an `or`
  reduction of the mask per vector iteration) and of `vector.early.exit`
  (`calculateEarlyExitCost`) is not added to the plan, so a VF may be chosen
  that the pin's prices would refuse.
- VPEE6 a port takes the loops only where `Tti::vectorizes_early_exit` says
  its backend selects the vector loop (x86-64).

The rows' owner is x87-cordic-2, exit 2026-12-10.

## Runs
Inside `loopvectorize`, where the planner meets a loop with an uncountable early exit.

## Example
A read-only search loop that leaves at the first match gets a vector loop in front of it; the original loop finishes the rest.

```text
for (i = 0; i != n; i++) if (a[i] == x) break;
=>
vector.body: %mask = icmp eq <VF x i32> %wide.load, %x.splat
             %any = reduce.or(%mask); br %any, vector.early.exit, vector.body.interim
scalar.ph:   the original loop, from %bc.resume.val
```

> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/genericsched.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/genericsched.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/genericsched.rs sha256:8c579bd4e611a5bc9ba3123b3304d30d4b72c50c7146c74e7634608103e2d7a9 -->

# genericsched

`GenericScheduler::tryCandidate`: how the pre-RA machine scheduler picks
between two ready nodes (row 65, S0 of ARM64-SCHED-HOIST).

## How it works
A candidate carries the node, the zone it would extend, the reason it won
so far and what it costs: pressure, resources, stall cycles, depth and
height. `try_candidate` compares a new one with the best so far through
the pin's list of heuristics in the pin's order (MachineScheduler.cpp
`tryCandidate`, #L3965): the first heuristic that tells the two apart
decides, and records its `Reason` on the winner (and, as `tryLess` and
`tryGreater` do, lowers the reason of a loser that stays).

```text
PHYS-REG  REG-EXCESS  REG-CRIT  [same zone: acyclic latency, STALL]
CLUSTER   [same zone: WEAK]  REG-MAX
[same zone: RES-REDUCE  RES-DEMAND  latency  ORDER]
```

The zone is `None` when the two come from different zones (top and bottom
of a bidirectional region): only the heuristics that compare across zones
apply.

## Reasons (pin d1106deb71cc)
The enum is [`CandReason`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/MachineScheduler.h#L1099); each reason is set at:

| reason | set at |
|---|---|
| `NoCand`, `Only1`, `FirstValid` | [reset](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/MachineScheduler.h#L1183), [`pickOnlyChoice` picks](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L4107), [first candidate](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L3970) |
| `PhysReg` | [`tryBiasPhysRegs`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L3895), called at [#L3975](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L3975) |
| `RegExcess`, `RegCritical` | [#L3981](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L3981), [#L3988](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L3988) (`tryPressure` [#L3805](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L3805)) |
| `Stall` | [#L4008](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L4008) |
| `Cluster`, `Weak` | [#L4026](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L4026), [#L4033](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L4033) |
| `RegMax` | [#L4040](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L4040) |
| `ResourceReduce`, `ResourceDemand` | [#L4048](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L4048), [#L4052](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L4052) |
| `TopDepthReduce`, `TopPathReduce`, `BotHeightReduce`, `BotPathReduce` | [`tryLatency`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L3488), called at [#L4003](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L4003) and [#L4058](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L4058) |
| `NodeOrder` | [#L4064](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L4064), and in `tryBiasPhysRegs` |

## Not here
The ready queues, the clock and the resource counts of a zone
(`SchedBoundary`) and the choice of zone (`pickNodeBidirectional`) are the
callers'; this module is given what they measure.

## Example
Two ready nodes are compared by the heuristics in order; the first that
differs decides and its reason is recorded on the winner (the values are
illustrative; the `RegExcess` case is the one the unit test below checks).

```text
cand X: REG-EXCESS 0, height 3
cand Y: REG-EXCESS 2, height 9
try_candidate(Y, best X)  -> X stays, reason RegExcess  ; height is never looked at
```

## Used by
Named by `genericdriver`, `machinesched`.

> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/combiner.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/combiner.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/combiner.rs sha256:8a24528e8df9d31229b70628b12de8947446e08877ee045065d80d33907461ae -->

# combiner

A chain of one integer operation rebuilt so the operand that arrives last
joins last: LLVM's `MachineCombiner`
([MachineCombiner.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineCombiner.cpp))
with the generic reassociation patterns of `TargetInstrInfo`, the only ones
i486 has.

## How it works
Each block is walked in order. An instruction is a root when it is
`add`, `and`, `or`, `xor` or `imul` on two registers
(`X86InstrInfo::isAssociativeAndCommutative`) with no live flags, and one
of its operands is the result of the same operation (`prev`) that this
root alone reads, also with no live flags, defined in the block
(`getMachineCombinerPatterns`, `isReassociationCandidate`,
`hasReassociableSibling`, `hasReassociableOperands`). With
`B = A op X` and `C = B op Y` the four patterns (`REASSOC_AX_BY`,
`XA_BY`, `AX_YB`, `XA_YB`) say which of `prev`'s operands is `A`; the
alternative is `N = X op Y`, `C = A op N` (`reassociateOps`,
`mustSwapOperands` for the operand order), both placed where the root was.
It replaces the pair when the depth of the new root is smaller than the old
root's (`improvesCriticalPathLen`, `MustReduceDepth`). A value's depth is
the largest `depth + latency` over the values its definition reads
(`getDepth`): a load takes 4 cycles, a copy 0, anything else 1
(`defaultDefLatency`, no scheduling model for i486).

```text
  t1 = a + load        n  = a + b
  t2 = t1 + b    ==>   t2 = n + load     (the load, 4 cycles, joins last)
```

## Example
The unit tests `a_chain_whose_last_operand_arrives_late_is_rebuilt` and
`an_intermediate_two_instructions_read_stays`.

## Runs
On SSA, after `leaopt` and before `phielim` (the pin's `addILPOpts`, after
`MachineSink`), at every -O level but -O0.

## Limits
`preservesResourceLen` is true at once (no scheduling model), and
`reduceRegisterPressure` and the throughput and accumulator patterns are
not X86's. Optimizing for size does not take the pin's shortcut.

DEVIATE MC1 (owner funcattrs-port, exit 2026-11-13): the trace is the
block alone (`MachineTraceMetrics` `TS_MinInstrCount` also follows the
predecessors and the physical registers: the stack pointer around calls
orders two calls there, and here nothing does); a value made outside the
block has depth 0 and the latency of its definition. Exit: a trace analysis
on LIR.

DEVIATE MC2 (owner funcattrs-port, exit 2026-11-13): the latency table is
the default one written above, read from the operands (a memory source is a
load); the pin asks the opcode's `mayLoad`.

DEVIATE MC3 (owner funcattrs-port, exit 2026-11-13): reads of a value are
counted from the operands and phis of the body (`hasOneNonDBGUse`), and a
value's one definition is found in a table made once, not
`MachineRegisterInfo`'s use-def chains. No kill flags exist in LIR, so none
are carried over, and the new instructions copy the old ones' fields except
the operands.

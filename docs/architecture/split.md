# The split

There is one line in this compiler and everything else is arranged around
it. On one side a program is bytes an 8086 executes; on the other it is
values and operations. **Nothing crosses except at the two doors.**

```
  ──────────── source ────────────┼──────── abstract ────────┼──────────── machine ────────────
                                  │                          │
  HIR (QB, Nib, C) ──▶ emit ──────┼──▶ llrm-transforms ──────┼──▶ isel ──▶ LIR ──▶ regalloc ──▶
  BC objects ──▶ decode ──▶ raise ┼──▶   pipeline            │    machine phases ──▶ layout ──▶ omf
                                  │                          │
                             the raise                   lowering
                             (one door)                  (the other)
```

## The rule

**Every pass between the raise and lowering takes MIR and returns MIR, and
names nothing about the machine.** No register, no mnemonic, no encoding,
no byte offset into a source object. Only selection and the machine phases
after it see machine form.

That is agents.md's fifth rule and this file is why it is not negotiable.

## Why

**A pass that knows the machine does the allocator's job badly.** The hoist
picked a spare register out of `regalloc.AVAILABLE`, rewrote every reader
to name it, and emitted the copies itself -- ninety machine references and
the source of every hoist bug for a year, including one that hung `harr` on
four configurations while the gate said clean. It had none of the
allocator's information and could not have had it.

**A pass that reads the emitted form asks the wrong question.** Twenty
callers asked `op.made or op.node.semantics` -- "what does this compute, in
x86" -- and for an operation a pass had already rewritten they got the
*original instruction*. A fold kept the fixup of the read it replaced, and
`tests/run/qb/jumps.bas` took the CASE ELSE arm for `k = 1`.

**An idiom recognised in a pass is recognised in the wrong place.** What a
long pair *is*, what an absorbable call *is*: those are questions about the
bytes BC wrote, and only the raise is looking at them. A pass that asks
gets a different answer each round, because by then the bytes have moved.

**And the abstract side is where the optimisation is.** Value numbering,
loop-invariant motion, promotion and induction variables are all statements
about values. Every one of them is impossible to state -- not merely harder
-- while a value is a register and an operation is an instruction.

## What each side may say

|                      | machine side | abstract side |
| -------------------- | ------------ | ------------- |
| a place              | a register, a frame slot | an SSA value, an `alloca` |
| an operation         | `ir.Operation`, a mnemonic | an LLVM instruction |
| a comparison         | flags, and six spellings of `jl` | `icmp` and its predicate, a value |
| a width              | the instruction's encoding | the type, `i16`, `i32` |
| a machine resource   | `Register.ES`, `st(0)` | an intrinsic, carrying the name and nothing else |
| where something is   | an address, a byte span | a block, and `br`'s targets |

A fact the machine side needs and cannot see in its own form, such as what
a call may touch, is computed on MIR and carried down on the instruction
(`model::lir::CallMemory`), never re-derived from machine form.

## How it is enforced

- **The crate graph.** `llrm-mir`, `llrm-analysis` and `llrm-transforms`
  depend on no machine crate, so a pass cannot name a register. The target
  reaches them only as `llrm_mir::target::Machine`: costs and counts.
- **Stage dumps.** `LLRM_MIR_STAGES=DIR` writes MIR between passes;
  `ISEL_DUMP=1` prints LIR after selection and after each machine phase.

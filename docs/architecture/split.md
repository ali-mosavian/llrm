# The split

For contributors deciding where a piece of logic belongs. There is one line in
this compiler: on one side a program is values and operations, on the other it
is machine instructions. **Nothing crosses except at the two doors:** MIR
construction and lowering.

```
 -------- source ---------+------- abstract -------+-------- machine --------
                          |                        |
 HIR (QB, Nib, C) -emit --+-> llrm-transforms ------+-> isel -> LIR -> phases
 BC objects -decode-lift -+->   pipeline            |   -> layout -> writer
                          |                        |
                    MIR construction            lowering
                    (one door)               (the other door)
```

## The rule

**Every pass between MIR construction and lowering takes MIR and returns MIR, and
names nothing about the machine.** No register, no mnemonic, no encoding, no
byte offset into a source object. Only selection and the machine phases after
it see machine form.

## Why

**A pass that knows the machine does the allocator's job badly.** Picking a
spare register, rewriting readers to name it and emitting the copies means
deciding placement without the allocator's information.

**A pass that reads the emitted form asks the wrong question.** For an
operation a pass has already rewritten, the original instruction answers
"what does this compute" wrongly.

**An idiom recognised in a pass is recognised in the wrong place.** What a
long pair is, what a runtime call means: these are questions about the source
form, and only the frontend (HIR emission, or the lift of a BC object) sees it.

**The abstract side is where the optimisation is.** Value numbering,
loop-invariant motion, promotion and induction variables are statements about
values. They cannot be stated while a value is a register and an operation is
an instruction.

## What each side may say

|                    | machine side             | abstract side                  |
| ------------------ | ------------------------ | ------------------------------ |
| a place            | a register, a frame slot | an SSA value, an `alloca`      |
| an operation       | a mnemonic, an encoding  | a MIR instruction              |
| a comparison       | flags                    | `icmp` and its predicate       |
| a width            | the instruction's form   | the type, `i16`, `i32`         |
| a machine resource | `es`, `st(0)`            | an intrinsic, by name only     |
| where something is | an address, a byte span  | a block, and `br`'s targets    |

A fact the machine side needs and cannot see in its own form, such as what a
call may touch, is computed on MIR and carried down on the instruction
(`CallMemory` in `crates/backend/llrm-core/src/model/lir.rs`), never
re-derived from machine form.

## Who owns what

| Concern | Owner |
| --- | --- |
| Object records and relocations | the object writers; for BC input, `llrm-omf`, `llrm-x86-bcmachine` |
| Language semantics, runtime idioms | the frontend and `llrm_hir::mir::emit`; for BC objects, the lift in `llrm-x86-bc` |
| Types, values, comparisons, memory objects and effects, control flow | MIR |
| CSE, DSE, promotion, LICM, ranges, loop recurrences | `llrm-transforms`, `llrm-analysis` |
| Runtime call effects (reads, writes, escape, return) | contracts in `llrm-qbruntime`, consumed by HIR emission and MIR |
| Argument locations, clobbers, stack cleanup, frames | physical ABI summaries, consumed by lowering and the backend |
| Encodings, addressing modes, timing trade-offs, operand constraints | selection |
| Phi realisation, parallel copies, tied operands, coalescing, spills | machine phases and `RegAlloc` |
| Redundant moves and segment reloads after placement | `Peephole` |
| Branch distances, layout, rewritten relocations | layout and `objbuild` |

Machine phases implement MIR efficiently; they do not rediscover CSE, LICM or
loop transformations on LIR.

## How it is enforced

- **The crate graph.** `llrm-mir` has no dependencies
  (`the_crate_depends_on_nothing`); `llrm-analysis` and `llrm-transforms`
  depend only on it and `llrm-support`. `llrm-core` names no
  `llrm-x86-m*` crate (`tests/core_depends_on_no_target.rs`). A pass cannot
  name a register.
- **Stage dumps.** `LLRM_MIR_STAGES=DIR` writes MIR between passes;
  `ISEL_DUMP=1` prints LIR after selection and after each machine phase.

## Goal

Every documented target is within 1.5x of an independently derived reference
under equivalent semantics. A refusal or an unchanged fallback is not an
optimisation win.

**Review question:** could this MIR transformation still be stated if the body
were lowered to a different machine? If not, move the decision across the
line.

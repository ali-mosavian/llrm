# ProcessImplicitDefs: the translation

Pin d1106deb71cc, [ProcessImplicitDefs.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/ProcessImplicitDefs.cpp) (`processImplicitDef`, `canTurnIntoImplicitDef`). Row 59. The premise ([process-implicit-defs-premise.md](process-implicit-defs-premise.md)) only sets the order: its whole effect in the pin is 0.04% of bytes at m16 and nil at m32. The pass is translated.

## The pass

For each `IMPLICIT_DEF` of a virtual register: every use (not a tied-and-other-read) becomes an `undef` read; a user that is a copy, an `INSERT_SUBREG`, a `REG_SEQUENCE` or a phi that reads nothing else becomes an `IMPLICIT_DEF` itself and is worked in turn; the def is erased when all its uses are undef. Physical-register implicit defs are not in LIR.

## What LIR lacks

| piece | today | needed |
|---|---|---|
| the def | none: an `undef` constant is the immediate 0 (`isel.rs`, `isel/wide.rs`, `isel/matcher.rs`: 8 sites match `ConstantKind::Undef`) | an `implicit_def` instruction: one value defined, nothing read |
| the read | `Insn.uses` lists every value an instruction reads; liveness gives each a read (`LiveIntervals::add_operands`: `reads_reg: true`) | an undef read: named by the operand, absent from `uses`; no live range starts or extends because of it |
| the register | a value with no definition has no interval (`LiveIntervals` panics "a unique value") | the rewriter gives an undef operand any register of its class, as `VirtRegRewriter` does |
| the user turned def | none | a copy/phi/extend whose only source is undef becomes an `implicit_def` |

## Order (each piece lands once it builds and passes the gate)

1. LIR: the undef read (`Insn` carries the values read as undef), liveness and the allocator's rewrite of one; unit tests of each with hand-made bodies, no producer. Wrong output cannot come from it: nothing makes one.
2. `implicit_def` and the pass (`processimpdefs.rs`, PHI-block handling as the pin), tests from the pin's cases: a use becomes undef; a copy of an undef is an `implicit_def`; a tied-and-other read keeps its def.
3. isel: an `undef` that needs a register is an `implicit_def` (the 8 sites), not 0. This is the change a program can see: a read of uninitialized memory is no longer zero. DEVIATE (zero) is deleted with it; the MIR undef work (user decision 2026-10-19) says when that is allowed, and it is checked with the bitfield cases of #2507 and #2521.

## DEVIATEs

- PI-1: no physical-register implicit defs. Exit: none; the target has none before allocation.
- PI-2 (exit: piece 3): until isel emits `implicit_def`, the pass has no input.

Dates: piece 1 2026-10-30, 2 and 3 2026-11-02. Owner w-1.

# Rich portable MIR

Status: steps 0 to 3 landed (`tools/baseline.sh`, `crates/llrm-mir`, the
rewrite ledger, LIR meta instructions, LLVM IR in `llrm-mir`); the compiler
still runs on the old MIR.

## Decision

MIR is LLVM IR. Its text is a subset of LLVM's assembly language, its
meaning is the LangRef's, and every MIR file is one that LLVM 20's
`opt -passes=verify` accepts. Where llrm needs something LLVM lacks, it
takes the form LLVM gives extensions: an intrinsic, an attribute or
metadata. It never adds syntax, a type or an instruction.

The form is LLVM's; the compiler stays llrm's. Its analyses, passes and
code generation remain its own, because LLVM has no segmented 16-bit x86
target. LLVM is a reference and an oracle, never a dependency.

That buys what a private IR cannot have:

- **Independent oracles**, pinned to LLVM 20. `opt -passes=verify` checks
  every stage dump. `lli -force-interpreter` runs what it can: not
  overflow, min/max or `fmuladd` intrinsics or unwinding, and a
  `@llrm.qb.*` routine only where a `.ll` model of it is linked in.
  `llc -mtriple=msp430` compiles all but exception handling for a 16-bit
  non-x86 target, and `opt -O2` on the same MIR is a quality reference.
  `tools/mir-oracle.sh` runs all three.
- **LLVM's answers.** Poison and its flags, phi semantics, memory
  attributes and exception edges are settled
  questions there; llrm does not answer them again.
- **Readers.** Anyone who reads LLVM IR reads MIR.

## Where llrm differs from LLVM

The complete list; everything else is LLVM's.

1. **Passes see no target.** LLVM passes read `TargetTransformInfo`;
   llrm's do not (the fifth rule); where LLVM consults it, llrm's pass uses
   the MIR cost model, and what depends on the target is lowering's.
   `target datalayout` stays, because here it states the program's memory
   layout: BC's object fixes it, and a C or Nib frontend states its own, as
   clang does. It names no native integer widths (`n`), and there is no
   `target triple`; lowering is given the target.
2. **A subset.** No `undef`, which LLVM is retiring for `poison`; no
   constant expressions outside global initializers; nothing without a
   producer, such as GC, coroutines or scalable vectors. The subset grows
   when a producer needs more, never by an llrm construct.
3. **Rust, not C++.** The in-memory form is LLVM's object model -- module,
   functions, blocks, instructions, typed values, use lists,
   `replace_all_uses_with`, and a pass manager whose analyses are cached
   and invalidated by `PreservedAnalyses` -- held in arenas addressed by id
   rather than by intrusive pointers. Ids are never reused. Erase, clone
   and replace record what they did, which feeds the rewrite ledger now
   that a pass's input no longer survives it; and, as LLVM's
   `-verify-analysis-invalidation` does, an instrument recomputes each
   preserved analysis and compares.
4. **Source bytes are not IR.** Which original bytes an instruction owns is
   the rewrite ledger's (step 2), outside MIR. Diagnostic lineage is
   `!llrm.origin` metadata; stripping it must not change the output, the
   rule LLVM holds for `-g`.

## llrm's needs in LLVM's terms

| Need | LLVM form |
| --- | --- |
| A long held in a register pair | `i32`, recognized by the raise |
| A write to half of a word | `trunc`, `zext`, `shl`, `or` |
| Carry, borrow, overflow | `llvm.uadd.with.overflow` and its kin, `extractvalue` |
| `B$MUI4` and other runtime arithmetic | the LLVM operation |
| Division | `sdiv`, `srem`, unguarded: division is C's (agents.md), so a zero divisor is undefined, as in C |
| An error check BC's code makes | `icmp`, `br`, a call to `@llrm.qb.error(i16)`: `noreturn`, and not `nounwind`, since a handler catches it |
| `ON ERROR`, `RESUME` | `invoke` and `landingpad` under `@llrm.qb.personality` |
| `GOSUB`/`RETURN`, `RESUME` targets, event handlers | one mechanism: an `i16` continuation index and a `switch` over the places control may return to |
| Event traps (`ON TIMER`, `ON KEY`) | a call to `@llrm.qb.events` at each check BC makes |
| An alternate entry sharing a frame | a master function taking an entry selector, and a thin function per entry, as flang lowers `ENTRY` |
| Variables BASIC zeroes | explicit stores or `zeroinitializer`; `alloca` is uninitialized |
| String space and dynamic arrays, which the runtime moves | reached through their descriptors; no pointer into them lives across a call that may allocate |
| Near and far pointers | `ptr` and `ptr addrspace(1)`; datalayout `p:16:16-p1:32:16:16:16`, a far pointer indexing by 16 bits; its integer form is segment:offset, so a near offset into far data is `ptrtoint` to `i16` |
| Huge-array address arithmetic | an intrinsic; segment arithmetic has no LLVM form |
| An object's fixed layout | the datalayout, struct types, globals and their initializers |
| A frontend's promise that an access stays in its object | `getelementptr inbounds`, `noalias`, `!alias.scope`, `!tbaa` |
| A runtime routine | `@llrm.qb.<name>`, declared with `memory(...)` and the rest as proved; few are `nounwind`, since most can raise an error |
| An unrecognized external call | a call to its declaration, all effects unknown; a function whose interface cannot be recovered is refused whole |
| `SINGLE`, `DOUBLE` | `float`, `double`, plain arithmetic; precision and floating exceptions are the machine's |

The raise recognizes; no pass does. `B$MUI4` is a `mul` the moment MIR
exists.

The analyses and passes are llrm's, shaped as LLVM's: dominators, loops,
scalar evolution, alias analysis, MemorySSA, known bits, value ranges, the
call graph. Each fact has one owner (the seventh rule).

## Stage dumps and the verifier

`tools/stages.py` writes one `.ll` per pass; diffing adjacent files is the
debugging evidence (the fourth rule).

`llrm-mir` implements LLVM's `Verifier` rules for its subset, so the
compiler needs no LLVM at run time. Its verifier and interpreter are
checked against `opt` and `lli` over the corpus: an input one accepts and
the other rejects, or an answer they disagree on, is a bug in one of them.

## Lowering contract

Lowering receives MIR, the target, the rewrite ledger and the ABI bindings
of an existing object. It may choose types' machine representations,
legalize, select, and implement intrinsics. It may not consult original
bytes to decide what an instruction means, invent an input, result or
edge, or feed a machine fact back into a MIR pass.

## Migration

The migration keeps the compiler runnable and output-stable.  Each issue found
gets a fail-first symptom regression in the same commit as its fix.

### 0. Freeze the baseline

- Record normalized MIR after every existing pass and final allocated output.
- Preserve byte-identical untouched OMF round trips separately from semantic
  reconstruction.
- Record linked answers, final assembly, structural metrics, and per-CPU costs.

`tools/baseline.sh` records every MIR, LIR and assembly stage of the OMF
corpus and the Nib examples; while final assembly is unchanged, so are costs.
The round trip is `test_round_trip_is_byte_identical` and
`test_every_mapped_fixture_round_trips_byte_identical`; answers are
`tools/e2e/e2e.py`.

### 1. Land the typed shell and its instruments

- Introduce `MirContext`, the first scalar types, typed constants/values,
  signatures, explicit parameters and returns, stable semantic IDs, and one
  small opcode family.
- Land the verifier, deterministic printer/parser, normalized stage dump, and
  a focused interpreter for that slice immediately.
- Make an undefined non-parameter use a verifier error.
- Add a temporary adapter from new MIR operations into the existing backend so
  the compiler remains runnable before lowering is ported.
- Derive compatibility widths from types temporarily; never infer a type from a
  consumer's guessed width.

No old field is deleted in this step.

Landed as `crates/llrm-mir`, which has no dependencies: MIR cannot name a
decoder, object file or target, and a test keeps it that way. The adapter and
compatibility widths wait for the first producer in step 3, since nothing
emits the new MIR yet. Its private text syntax gives way to LLVM's in
step 3.

### 2. Separate lineage from byte ownership

- Make diagnostic lineage many-to-many and discardable.
- Introduce mandatory `TransformChange` records.
- Move retention, replacement, deletion, refusal, landing points, and
  relocation intent into an independent rewrite ledger.
- Prove that cloning and DCE no longer require live instruction anchors.

This happens before opcode migration changes transformation identity and before
new DCE/CSE can duplicate the old ownership coupling.

In the old MIR, `Op.id` is each operation's own identity: unique in its body,
assigned by the pass manager (`mir::identified`) after the raise and after
every pass, and excluded from equality. `Op.source` is the provenance a copy
shares: the raise-time operation whose relocations, node and site it
re-emits. `mir::transformed` identifies a pass's output and diffs it against
its input into `TransformChange` records (rewritten, cloned, deleted,
inserted); `transform::recorded` returns them per stage with the body, the
unroll, peel and unswitch drivers included.

No pass sees a tombstone. At each pass boundary `mir::transformed` strips
them and records where their bytes land: before the next operation still in
the block, chained as that operation is deleted or moves, or where a vanished
block's control or last operation went (`mir::Ledger`). `transform::applied`
puts them back before the backend runs.

In LIR the byte markers are meta instructions (`Insn::is_meta`), as LLVM's
debug instructions are: they take no slot, and no pass counts, windows or
stops on them. `LLRM_STRIP_META` drops them at every LIR phase boundary;
the code must come out the same, as LLVM's must with and without `-g`.

### 3. Make `llrm-mir` LLVM's

- First the instruments: the harness that prints MIR, has `opt` verify it
  and compares llrm's interpreter with `lli`, with a wrong fixture seen
  failing; and hand-written `.ll` for division, `ON ERROR`, `GOSUB`,
  `ON TIMER`, a far array and floating point, run through `opt`, `lli` and
  `llc` to prove the mappings above before code depends on them.
- Parse and print the subset of LLVM's text; step 1's private syntax goes.
- Hold it in LLVM's object model: use lists, `replace_all_uses_with`, the
  pass and analysis managers, and change records for the ledger.
- Verifier and interpreter for the subset, checked against `opt` and `lli`
  on the fixtures.
- Modules: the datalayout, declarations and attributes, globals and
  initializers, LLVM's intrinsics.

The text landed first, with the arenas it reads into: `llrm-mir` parses and
prints the subset, and `tools/mir-oracle.sh` checks that LLVM reads its
output as it reads the input, over the mapping fixtures and clang's output
for `bench/parity`.

Then the mutation API (`llrm_mir::edit`): LLVM's `setOperand`,
`replaceAllUsesWith`, `eraseFromParent`, `clone` and moves, over use lists
kept for values and blocks, each change logged for the ledger;
`check_uses` recomputes the lists and compares.

Then the verifier (`llrm_mir::verify`), over a dominator tree: LLVM's
rules for the subset. `tools/mir-oracle.sh` holds it to `opt`'s verdicts,
accepting what `opt` accepts and refusing `tests/invalid` for the reasons
they state.

Then the datalayout and the interpreter (`llrm_mir::interpret`): LLVM's
semantics with poison, memory laid out as the datalayout says, and
undefined behaviour reported rather than run. On every fixture that states
its answer it agrees with `lli`.

Then the pass manager (`llrm_mir::passes`): function passes over a module,
analyses cached per function until a pass's `PreservedAnalyses` drops
them, and each pass's changes returned as a stage for the ledger. Its
instruments are LLVM's `-verify-each` and `-verify-analysis-invalidation`,
which recomputes every analysis a pass kept.

Then LLVM's intrinsics (`llrm_mir::intrinsics`): one table gives each its
signature and attributes, which the parser sets as LLVM does, the verifier
checks with LLVM's messages, and the interpreter runs. Those lli cannot run
are checked by hand. The `@llrm.qb.*` routines are declared by the raise,
which proves their attributes, so they come with step 4.

### 4. Raise into MIR beside the old MIR

- The QB raise and the C and Nib frontends also build a MIR module. Nothing
  consumes it yet.
- Every corpus function's MIR passes `opt -passes=verify`, or is refused
  whole with the reason.
- A lint checks that raised MIR makes no poison BASIC defines, before any
  pass can exploit it.
- An adapter lowers MIR through the old backend, so the e2e answers check
  the new MIR, and `lli` agrees with them where it can run.

HIR, the common frontend of QB source and Nib, emits MIR first
(`llrm_hir::mir`), as clang's CodeGen emits LLVM IR: data objects become
globals, local places allocas, calls to routines the module does not
declare `@llrm.qb.*` calls. `tools/hir-mir-corpus.sh` emits the QB suite,
has `llrm-mir` and `opt` verify each module, and counts the refusals by
reason.

### 5. Port the passes

- One pass at a time onto MIR, against `.ll` fixtures and the interpreter.
  Where LLVM has the pass, llrm's follows it.
- Representation changes stay apart from optimization-policy changes.

### 6. Lower from MIR

- Lowering reads only MIR and the side tables; step 4's adapter goes after
  output parity.

### 7. Delete the old MIR

- `Op`, `MemRef`, the decoded-operation fallbacks and their constructors, in
  one bounded cleanup. Update `docs/architecture/split.md`,
  `docs/architecture/mir-vocabulary.md` and the diagrams.

## Acceptance gates

Every step passes those that apply:

1. Untouched OMF read and write stays byte-identical.
2. Unoptimized decode, raise, lower, allocate, emit, link and run keep the
   program's answers.
3. Optimized code and answers stay unchanged until an optimization is
   proposed and measured on its own.
4. Each fixed symptom and each fixed instrument has a test seen failing
   first.
5. The first differing adjacent stage names the responsible pass.
6. Every stage of the corpus passes `opt -passes=verify` and llrm's
   verifier alike.
7. llrm's interpreter, `lli` and the linked program agree.
8. Stripping `!llrm.origin` changes no output.
9. The rewrite ledger accounts for every source byte, fixup and external
   landing point exactly once.
10. Quality is judged by `docs/measurement/targets.md`; MIR is
    infrastructure, not evidence of better code.

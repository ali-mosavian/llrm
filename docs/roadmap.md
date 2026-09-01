# Roadmap

Kept current. Tick a box when it lands, and move a number when it is
re-measured -- a roadmap that lags the code is worse than none, because it
is read as if it did not.

## Goal

Recompile BC's output rather than patch it: integer and long arithmetic,
floats, array access, loop invariants, strength reduction, register
reallocation, constant folding, CSE, dead code elimination.

## Rule

**MIR is the pass. The machine-code arm is legacy.**

`lift.py`, `calls.py`, `forward.py`, `memory.py` and `fpu.py` rewrite
regions of decoded instructions in place. They stay gated and their bugs
still matter, but they get no new transforms: a peephole over emitted bytes
has no representation to do CSE, hoisting or reallocation in. Each is
retired when MIR expresses it (M5). MIR is finished when none are left.

## Blocker — cleared

It read: `mir.lower()` returns each op's `node` verbatim, so MIR can delete
and can emit the one instruction `select.py` knows, but cannot rebuild a
body -- and CSE must emit a copy, folding an immediate, LICM a move between
blocks.

That is done. `rewrite.py` writes the code segment from MIR, an `Op` can
carry semantics no node produced (`Op.made`) and say which of the original
bytes it stands for (`Op.covers`), and `layout.py` places the result and
relaxes its branches. What orders the milestones below now is narrower:
**nothing moves code between blocks yet**, and `transform.placed()` moves it
within one but is switched off.

## State

Measured 2026-09-01. Re-measure before trusting it.

| | |
|---|---|
| corpus | 98,738 → 92,736 code bytes, **-6.08%**, over 170 objects |
| qb-qrender | 74,979 → 75,535 code bytes, **+0.74%** — absorption costs the bytes, emission and the peephole give most back |
| **qb-qrender runs** | 291 frames against BC's 271, same geometry. It did not run at all before this session |
| its arithmetic calls | 157 → 1 |
| its x87 sites | 1,766, none optimised |
| bench/nbody | 2.99× under DOSBox, 21 of 21 calls absorbed, pressure 6/6 |
| corpus redundant reads | 751 — 73 live provider, 48 dead, 630 none |
| selector coverage | 24,695 of 24,739 corpus ops; the 44 are `movsw`, refused deliberately |
| segments MIR wrote | **every one**: 170 corpus objects and all 246 of qb-qrender's BC-built ones, absorbed or not |
| **optimised and MIR-written** | **the default**, 19 suite programs on all 12 configurations, the fuzz corpus clean, and 40 of 40 mutations |

MIR does, end to end:

- [x] raise to SSA and lower back byte-identical
- [x] variables that are not registers — `docs/variables.md` stage 1
- [x] half and byte access — stage 2
- [x] the stack as an address space — stage 3
- [x] prove an identity and delete what computes nothing — stage 4
- [x] substitute a memory operand for a register, cross-block
- [x] emit the register and immediate forms from `ir.Semantics` — `select.emit`

Built, no consumer:

- [ ] `consts.known()` — proves 1,971 corpus values, nothing emits from them
- [x] `wide.py` — superseded by `qbopt/pairs.py`, which is on. See M5
- [ ] `regalloc.colour()` — correct, and cannot pay while identity is optimal at pressure 6/6

## M1 — instruction selection

`lower()` emits a body it was not handed. Everything else waits on this.

- [x] `mov reg,reg` and `mov reg,imm`
- [x] add, adc, sub, sbb, and, or, xor, cmp — reg-reg and reg-imm
- [x] neg, not, inc, dec
- [x] push, register and immediate, at both widths
- [x] refuse what is not in the table rather than approximate it
- [x] load, store, push, accumulate and store-immediate against `Space.SEGMENT` and `Space.FRAME`
- [x] cmp, cwd, cdq, wait, nop, pop, ret and retf
- [x] near call, far call, jump and conditional branch
- [x] `imul` — `select.multiply_into`; `imul eax,ecx` is `66 0f af c1`.
      Absorption is what emits it, so BC's own output has none to count
- [x] the x87 instructions — 869 of them select across the corpus.
      Their *stack positions* are still not values, which is M6's
- [x] the addresses `operand_of` refused — none left: every operation in
      the corpus selects except `movsw`, which is refused deliberately
- [x] layout: a whole body emitted, with every branch pointing at where its
      target went — `layout.lay_out`, 50 of 171 corpus bodies, 8,353 ops and
      583 branches, all landing
- [x] pick the shorter encoding where it fits — short branches to a fixed
      point, the accumulator's moffs load and store, one-byte inc and dec,
      byte-sized immediates. 6.3% larger than BC became 4 bytes smaller
- [x] the five BC writes and this did not — the byte-immediate push, a byte
      displacement through a base register, the accumulator's own arithmetic
      opcode, the by-1 shift, and the byte immediate in a memory compare.
      Whole-segment emission cost qb-qrender +3.6% before these and saves
      bytes after
- [x] `cmp reg,0` as `test reg,reg` — a byte shorter and the same question,
      and the only peephole here on this pass's *own* output: BC writes no
      `cmp reg,0` at all, so every one is absorption's. Not when the
      immediate is relocated, where the zero is an address
- [x] an immediate a fixup names keeps its width — `add ax,offset X` arrives
      as `add ax,0`, and the byte form fits zero. `select.emit` had the flag
      and none of its callers passed it, so a two-byte relocation named a
      one-byte field and the linker wrote over the instruction after it
- [x] a comparison keeps its mnemonic — `test` and `cmp` are both
      Operation.COMPARE and `test reg,reg` came back as `cmp reg,reg`, which
      sets ZF unconditionally. Latent until the peephole above would have
      been the first thing to emit one

Proof: re-lower every corpus body through the selector with no transform
applied; `tools/matrix.py` green. Same program, not same bytes — the
selector may choose differently.

Held so far without running anything: everything emitted decodes to the
instruction it was asked for (`test_everything_selected_decodes_to_what_was_asked_for`),
and every laid-out body keeps its instruction order and lands every branch
(`tests/test_layout.py`).

### Emission is whole-segment, not per-body

Measured when wiring `layout` into `rewrite`, and it changes the shape of
the rest: splicing a laid-out body back into BC's own layout works for
**1 of 171 bodies**. 33 cross a LEDATA boundary, 16 are not contiguous, and
the rest are branched into or carry a line number. BC's chunking is not
something a body-sized edit can step around.

Regenerating the whole code segment does not have the problem, because
nothing is being spliced: chunk boundaries, line numbers and fixups are all
being written rather than preserved. **Every one of the corpus's 170 objects
now has every body layable**; it was 42 when this was written.

- [x] `layout.rebuild` — every body laid out into one image, cross-body
      targets resolved, and all 4,846 fixups in the span carried. 34 of 125
      objects; the rest refuse on an op `select.py` cannot emit, or on data
      BC put inline
- [x] turn that image into records — `relocate.as_records`, `qbopt/wholeseg.py`.
      The code block goes where the LAST code LEDATA stood: OMF numbers
      externals by EXTDEF order, and a block written at the first one names
      externals whose EXTDEF has not been read, which LINK rejects outright
- [x] **MIR-written code links and runs** — `arith`, `cmpord`, `flags` and
      `nots` on PDS /G2 and QuickBASIC /O, in `tests/test_e2e.py`
- [x] the ON GOTO tables BC puts between instructions — carried verbatim
      with their fixups, since the entries are relocated words
- [x] BC's trailing zero padding, carried rather than selected — every
      object ends with `00 00 00 00`, which reachability walks into and the
      decoder reads as `add [bx+si],al`
- [x] a cell whose address cannot be named — `ir.Mem` keeps the register it
      is reached through and the displacement, out of the comparison, so it
      can be encoded without changing what "the same cell" means
- [x] the last object that refused, on bytes reachability never reached —
      `jumps-q-evt`'s two calls to `B$EVCK` sitting after an unconditional
      jump, real relocated code that nothing can arrive at. Carried the way
      padding and tables already are: a `Table` remaps every fixup inside it
      by however far it moved, and what it cannot do is fix a branch landing
      in its middle, which is exactly what reachability rules out

**Whole-segment emission is on.** `rewrite.py` writes the code segment from
MIR: 170 of 170 corpus objects and all 15 of qb-qrender's BC-built modules
rebuild, all nineteen suite programs run right on all twelve
configurations, and the fuzz corpus finds no divergence. It is not a size
cost either -- 175 bytes saved over the corpus and 593 over qb-qrender
against the patched output.

Two bugs stood between building it and it being correct, and neither the
host suite nor the twelve-configuration matrix could see either. Both took
the fuzz corpus, and then reducing a generated program from 173 lines to 31
with the machine build as the oracle.

- [x] **the relocated immediate.** `add ax,offset X` arrives at the selector
      as `add ax,0`, the sign-extended byte form fits zero, and the two-byte
      fixup then named a one-byte field -- the linker patched two bytes
      regardless, over the immediate and the byte after it, and a generated
      program read 0 for an array element. `select.emit` had a `relocated`
      flag for exactly this and none of its callers passed it.

      Nothing that compared the emitted code could see it: before linking,
      both forms disassemble as `add ax,0`. It took reducing the program to
      thirty-one lines and diffing the two linked images, where one says
      `add ax,0DCh` and the other `add ax,0FFDCh`. No fixture has the
      instruction -- BC writes it only for an array reached by adding its
      own address into ax -- so the regression test checks the wiring
      instead: `layout` tells the selector, on every call it makes
- [x] **the accumulate read as a load.** Letting `loaded_into` allow the
      high half a narrow write preserves also let `sub ax,[x]` through:
      both are one use whose origin is the destination's own register, and
      values alone cannot separate them. `redundant()` deleted
      `sub ax,ds:[0]` and `adc dx,[si+2]`. The semantics can separate
      them -- a binary operation names its destination among its sources
      and a move does not -- which is what `forward._loads_only` exists to
      stop, arrived at from the other side. No fixture reaches the shape,
      so a unit test on `_preserved` is what discriminates

## M2 — placement

An emitted instruction goes where it is needed, not where the old one stood.

- [x] a definition moves within its block, bounded by its own uses —
      `transform.placed()`, sinking to just before the first use. Built and
      switched **off**: it is the only transform here that changes the order
      instructions run in, and its benefit is indirect
- [x] what stops a move: nothing between may write what the op reads, and
      nothing may touch the register it writes. Values are per-definition
      and registers are shared, which is the part SSA hides

Proof: nbody's two `ecx` rejoins stop being special-cased, and the "the
source is gone by then" refusals in `simplify.py` and `avail.py` disappear.

## M3 — the classic passes

Each needs M1; anything that moves code needs M2. **All of them measure
empty on BC's output**, which reorders the rest of this file: the wins are
in absorption, not in the textbook passes.

- [x] constant folding — measured, not built: 1,086 results are known
      constants and every one comes out *longer*, because `xor ax,ax` is
      two bytes and `mov ax,0` is three
- [x] dead code elimination — measured, not built: BC emits none
- [x] CSE — measured, not built: 0 sites over SSA values, because BC
      reloads from memory rather than recomputing, and the memory
      redundancy `avail.py` finds is the same thing by another name
- [ ] LICM — `loops.py` has the structure and no consumer. Unmeasured
- [ ] array access — the index computation is the invariant worth hoisting

## M4 — registers

- [ ] `regalloc.colour()` reaches emission
- [ ] something that creates or relieves pressure, since identity is optimal until it does

### Segment registers are a register class, not scenery

`mir.PHYSICAL` excludes them and `_memrefs` drops the segment of a
`Space.FAR` address on the floor -- "a segment register is physical, never
a value". That is wrong for far pointers and has to change: es holds a
`$DYNAMIC` array's base, and two live far pointers are two values that
need two registers, not one register reloaded between every access.

Measured: qb-qrender has 1,294 instructions that touch es and 397 carrying
an explicit override, against 2 in the whole fixture corpus. So nothing
here will surface it and only a real program will.

**And a real program says the allocation is not the win.** Of qb-qrender's
621 `mov es,<x>`, every block loads es from a single source -- zero
alternation between two far pointers, which is what a second segment
register would buy. 42 are provably redundant: the same source, its base
register untouched, no store and no call in between. A naive textual match
says 283, which is what measuring the easy way costs.

So the work is deleting a reload, not allocating a class:

- [x] the redundant reload — `qbopt/segments.py`, without making es a value.
      42 sites, about 170 bytes in 26,290 instructions. Making it an SSA
      value would change what every op uses and defines, what regalloc has
      to colour and what select has to emit, for an optimisation nothing
      has been found to need
- [x] carrying `Space.FAR`'s `segment` through emission — `lift.memory()`
      named Space.SEGMENT, FRAME and GROUP and let FAR fall into a bare
      operand, so a widened pair read and wrote `ds` where BC wrote `es`.
      **qb-qrender linked and then corrupted itself**, on one module of
      fifteen, and `select.py` had handled the same address correctly all
      along. The mutation that guarded this had lapsed -- see the Gates
- [ ] `Space.FAR`'s `segment` as a *value*, so two pointers through
      different segments stop being one aliasing pair. Carrying it is not
      the same as tracking it: nothing yet knows two far pointers apart
- [ ] a second segment register, only if a program is ever found that
      alternates. None here does

## M5 — retire the machine arm

Done when the old path is deleted, not when MIR also does it.

Three of the five are built and on, and they find nothing -- **which is not
parity, and was read as parity here for too long.** `rewrite.py` runs
`forward.py` and `memory.py` first, so of course the MIR versions find
nothing left. Nobody had measured the other direction.

Measured now, on BC's untouched bodies: redundant loads 36 against MIR's
30, dead stores 43 against 36. Switching the machine deletions off costs the
corpus **45 bytes**, and the 13 sites are two known design limits rather
than bugs:

- MIR's `dead_stores()` is block-scoped and starts empty at each block's
  end, which its own docstring says costs a store that spans an edge. All
  seven misses are BC's module init, where the overwrite is in a later block
- the six load misses are a `mov ax,[x] / mov dx,[x+2]` pair, the partial
  write `avail.py` already has the hardest time with

`memory.py` cannot go on its own account either: `substituted_reads()` uses
`redundant_loads()` for the cross-block operand substitution, which is a
third consumer and has no MIR equivalent at all.

So the deletion is the milestone and it is further off than this file said.

Cross-block dead stores were the obvious suspect and are **not** the
blocker: `avail.dead_stores()` carries what its successors have overwritten
now, intersected over every edge, and the count does not move. Two things
were found on the way -- a clean call still wiped the map, because mir.py
gives every call a `MemRef(addr=None)` in loads and stores and the
clearing below ran on it anyway; and the real obstacle, which is that a
`push` is not a store `stored_from()` names, so it clears through
`op.stores` instead -- and its stack cell may alias a static, because BC
runs with `SS == DS`. Four pushes ahead of a call wipe everything known.

Two of those three are closed: a push's own stack cell no longer clobbers a
static (`may_alias` says it may, because BC runs with `SS == DS`, and that
is only true of a program whose stack has already grown into its data --
`memory.py`'s own docstring makes the same argument), and a store of a
constant is tracked at all, which `stored_from()` could not name because it
answers for the cell *and the value* and `mov word [x],1` reads none.

Two more followed from looking at what the *pipeline* hands the pass rather
than at BC's untouched bodies. The restore idiom was clearing the map as a
barrier, and it is a barrier for values only -- `ir.RESTORE_EFFECTS` reports
no load and no store. And a long written as one `mov [x],eax` covers both
halves BC stored separately, which `same_bytes` does not match: absorption
emits exactly that shape, so most of what `memory.py` found and this did not
was a narrow store inside a wide overwrite.

Dead stores are 42 of `memory.py`'s 43 on BC's own bodies, from 36, and
**the end-to-end cost of the deletion is 27 bytes, from 45**. All of it is
in the four inherited fixtures, whose provenance `mkfixtures.py` records as
lost; nothing else in the corpus loses a byte.

**None are left to build.** Widening is on, and absorption emits all four
routines at parity with `calls.py` -- 1,151 of 1,151 corpus sites, twelve of
twelve configurations under `--no-absorb-calls`. What remains of M5 is the
deletion itself, which is the milestone: `lift.py`, `forward.py`,
`memory.py`, `calls.py` and `fpu.py` all still run by default and all still
have to be measured with their MIR counterparts carrying the load alone.

### What the pair analysis is for

`qbopt/pairs.py` reads BC's two long register pairs over values, and five of
its shapes agree with `lift.py` exactly, object by object -- load 533,
alu-m 363, alu-i 121, not 51, move 12 across the corpus. It is easy to
mistake it for widening's prerequisite and judge it on what widening saves.
That is the wrong measure. **The goal at the top of this file is to
recompile BC's output, not to widen longs**, and knowing which values a pair
holds is what any of it needs -- array access, hoisting, reallocation, and
the integer work as much as the long.

Widening's own numbers, since they were measured and are worth keeping:
capping it costs the corpus **1,766 bytes** (89,806 against 91,572, about
1.9 per cent of the code) and qb-qrender only **196**, where **156 regions
are refused because widening them would grow the code** -- four times as
many as it takes profitably. That is the same fact as "qb-qrender declares
no LONG at all", seen from the other side.

The design consequence is worth stating before anything is built: M5's
widening is three parts, not two. Recognition, the rename, **and a cost
model** -- and on the program that matters most the cost model rejects most
candidates. `lift.py` has one ("widening it grows N bytes to M"). A MIR
version without it would make qb-qrender bigger.

- [x] widening — **on**, and clean on the fuzz corpus, all twelve
      configurations of `matrix.py`, and 20,813 host tests. Recognition, a
      rename, a restore and a cost model, all in `qbopt/pairs.py`;
      `transform.widened()` applies it, last, after the passes that reason
      about memory -- a widened op keeps the low half's own `loads`, two
      bytes at `[x]`, while the instruction reads four, and `avail.py` was
      forwarding a stale high half across it.

      **What it saves on top of what is already there is 21 bytes**, over
      the whole corpus. That is not a disappointment, it is the M5
      criterion: `lift.py`'s widening runs first and takes 489 of the 493
      chains BC's own bodies hold, so the MIR version finding almost
      nothing left is the same parity `forward.py` and `memory.py` already
      show. What it is worth is that `lift.py` can be deleted. **Measuring
      it against `lift.py`'s widening switched off has not been done**, and
      that measurement is the milestone, not this checkbox.

      It was written wrong twice more, and neither the host suite nor the
      twelve configurations could see any of it. The fuzz corpus caught all
      four; each has a regression test that fails when its fix is reverted:

      - **the constant.** BC splits a long across the two instructions, so
        `and ax,0ffffh / and dx,7fffh` is one `and eax,7fffffffh`. Widening
        the low half's semantics keeps `0ffffh` -- a different constant, and
        one that clears the high half of everything it touches. 224 of the
        corpus's pairs carry a high half that is not zero. `lift.py`
        combines them, and has all along
      - **the sign extension.** `mov ax,[x] / cwd` widened from the low half
        alone is `mov eax,dword [x]`: four bytes read out of a two-byte
        cell. `movsx eax,[x]` is the instruction and `select.py` has no form
        for it, so a chain neither starts on one nor spans one
      - **where a chain may start.** Only a load puts all thirty-two bits in
        one register. The slot being *known* says the value is tracked, not
        that it is in one register, and reading it as the latter started 13
        chains on arithmetic over a high half nothing had widened
      - **the restore's own address.** It went four bytes back from the end
        of the chain, to make its `covers` the four bytes it emits -- which
        is the address the last low already holds whenever the chain ends in
        a two-and-two pair. `layout.py` keys every op by address, so the two
        collided. What an op emits and which of BC's bytes it stands for are
        separate questions, and `layout.py` now measures a restore by the
        first. `suite/negnot.bas` is that shape, added because no fixture
        had it -- and with it the corpus is 170 objects, which moved every
        measured count in this file

      Two more had to be right and are worth stating because neither is
      about widening as such. A negate is three instructions --
      `neg ax / adc dx,0 / neg dx` -- and a `Pair` names two, so a chain is
      replaced by span rather than by member. And the restore is a barrier
      defining and using nothing: it used to be built by replacing the op
      before it, which handed it that op's own SSA values
- [x] recovering the arguments — `mir._stack_slot` keeps the depth
      across a *recognised* call, which is the half that works: an argument
      pushed before some other call runs is stranded under it, and its slot
      is only nameable if the depth crossed that call.

      `transform.arguments()` on top of it is now `stack.frames()` plus
      `calls.grouped()`, and names **1,151 of the corpus's 1,151** sites,
      agreeing push for push with `calls.py` on every site where both keep
      a push list. It is worth recording what it was, because it had a tick
      against it: a walk of its own that named 84 and got all 84 wrong, for
      three separate reasons:

      - **an unrecognised call ends the block's depth.** `CONSUMES` knows
        the four arithmetic routines and nothing else, so `B$PSSD`,
        `B$PEI4`, and `/V`'s own event-check stub all give up -- and 1,017
        of the 1,151 sites sit after one. The stub is the cheapest of these
        to fix and the most valuable: it is `cmp / jne / ret` on one path
        and `pop ax / push cs / push ax / jmp far B$EVCK` on the other,
        both stack-neutral, and it sits between almost every statement a
        `/V` build emits. All three `/V` configurations recover zero
      - **a memory push is invisible.** `live[slot]` is only written where
        the pushing op has exactly one non-flag *use*, which is a register
        push. `push word [x]` has none, and 5,492 of the corpus's 8,401
        pushes are that shape -- so the operand BC loads straight from a
        variable is never seen
      - **it counts slots, not operands.** `wanted = 2`, and a long is two
        pushes. At `chain-p-g2.obj:0x90` the two values it returns are the
        high and low halves of one long -- and of the long stranded there
        for the *next* call, not either operand of this one

      **And `qbopt/stack.py` already answered all three**, and did before
      any of that was written -- a virtual stack per block, a closed
      allowlist of push widths, a recognised call popping `4 * arity`, and
      an unknown one resetting rather than guessing. `calls.py` has used it
      all along. The MIR walk was a second, worse copy of a model already
      in the tree, which is what made it wrong in three ways at once
- [x] **emitting the absorbed call** — all four routines, 1,151 of the
      corpus's 1,151 sites, and at parity with `calls.py`.

      `transform.absorbed()` turns `call B$DVI4` into
      `pop eax / pop ecx / cdq / idiv ecx` and the restore after it, leaving
      the pushes where they are to feed the pops. That is `calls.py`'s own
      consume strategy, and popping rather than reloading is what makes it
      sound at a site whose pushes are not contiguous: `stack.frames()`
      proves the four bytes of each operand are the topmost region of the
      stack, and reloading one from its address instead would leave its push
      standing and leak four bytes per call, forever.

      **`B$CPI4` comes out byte-identical to `calls.py`'s ten instructions**,
      and a test says so rather than a reading of them. That routine changes
      no register at all, so absorbing it must not either: bp stands in as a
      frame pointer just long enough to name both arguments in place, edx
      holds one side, and both are put back without writing a flag the `cmp`
      just set -- the saved bp read before sp moves past its slot, because
      DOS services interrupts at any instruction boundary onto whatever
      stack is live.

      **It needed a lever to be testable at all.** `calls.py` absorbs every
      arithmetic call before a body reaches the MIR tower, so with the
      machine arm on the emitter finds nothing and no gate exercises it.
      `--no-absorb-calls` on `rewrite.py`, `matrix.py` and `fuzzcheck.py` is
      that lever, and it is how the machine arm gets retired. Under it,
      **all twelve configurations pass all nineteen suite programs** and the
      fuzz corpus finds no divergence.

      `cmpof` is the one worth naming: it exists because an absorbed `cmp`
      deliberately diverges from the runtime it replaces, and it passes --
      which is the sharpest statement of parity available, since it is the
      program that fails if the two arms disagree about the divergence.

      The flag gates are `flags.py`'s analysis, not MIR's own values: MIR
      has one FLAGS pseudo-register and cannot say *which* flag, which for
      the comparison is the whole question. The three arithmetic routines
      refuse on any flag read after the site; the comparison refuses only on
      CF, PF and AF. **No site in the corpus reads one**, which is why all
      1,151 are taken -- so the gates are tested by driving the analysis
      rather than by waiting for a shape the corpus does not have.

### A transform may emit more than it replaces

The thing that had to change first, and it was load-bearing for two
unrelated routines. `layout.py` keyed `lengths`, the short-branch set and
the placement map by `op.at`, so no two ops could share an address -- and a
far call is five bytes however many operations replace it. `B$RMI4` needs
six and `B$CPI4` ten.

- [x] place by position, not by address — `_placed` returns where each op in
      the list lands *and* what each address means afterwards, the latter
      keyed by the **first** of a group, so a branch to a call arrives at
      the start of what replaced it rather than into the middle. The ops
      themselves keep the call's own address, one of them standing for its
      five bytes and the rest for none, and the byte arithmetic still adds
      up
- [x] a restore has no field for a fixup — `_field_in` searched the op's
      span, and the restore idiom is put wherever a transform has an address
      to spare: on a far call's own byte, whose target is a fixup, or on a
      chain's last high half, which may be a store through a relocated
      displacement. Both would have been found and neither belongs to
      `push eax / pop ax / pop dx`. Latent for widening, where it would have
      refused the body rather than corrupted it, and reachable for the first
      time here

- [x] load forwarding — `avail.redundant()`, 12 of 12. It also needed the
      register's current value tracked: `holders()` maps a cell to the value
      put there and says nothing about whether that value is still in its
      register, so after `mov ax,[x]` then `mov ax,[y]` the entry for `[x]`
      still names a value whose origin is eax. What blocked it first was
      `loaded_into` refusing a partial write: `mov ax,[x]` writes sixteen
      bits of a thirty-two bit variable, so the high half survives and MIR
      records a read of the old `eax`. A real read, and not the instruction
      consulting memory, which is the question being asked -- so all 36
      sites came back "not a plain load" and none was anything else.

      Allowing that partial write then let an *accumulate* through, because
      `sub ax,[x]` reads the old eax the same way: one use whose origin is
      the destination's own register. `redundant()` deleted
      `sub ax,ds:[0]` and `adc dx,[si+2]` before the semantics were asked
      instead of the values -- a binary operation names its destination
      among its sources and a move does not
- [x] dead stores — `avail.dead_stores()`, block-scoped and starting empty
      at each block's end, which costs a store that spans an edge and can
      never invent one that does not
- [x] native x87 — `layout` selects an emulator site from its own semantics
      under `native_fpu` rather than carrying its bytes

- [ ] **and `--native-fpu` does not work on qb-qrender.** It hangs, and both
      halves of the fifteen modules hang independently, so it is systematic
      rather than one module. `bench/fpbench.bas` runs correctly under the
      same flag and the same DOSBox (1.88x), so the flag is not simply
      broken.

      Unexplained, and left that way deliberately: the flag is opt-in, says
      REQUIRES A COPROCESSOR, and is not in the default path, so this is a
      limitation to know about rather than a bug blocking anything. The
      guess worth testing first is that BC's `/FPi` emulator patches its own
      `int 34h`..`3Dh` sites at load time when a coprocessor is present, and
      that pre-patching some of them interacts with whatever qb-qrender's
      uGL library does with the FP stack -- 1,766 sites there against
      fpbench's handful

## M6 — floats

Largest untouched surface, newly testable: `fuzzgen.py` generates SINGLE
and DOUBLE, and `87bhelp.asm`'s six helpers have contracts.

- [x] the x87 memory and popping forms select — `fld [x]`, `faddp st(i),st(0)`
- [x] x87 stack positions as MIR values — `qbopt/fpstack.py`. Entering slots
      are minted rather than assumed empty: BC leaves values on the stack
      across a branch
- [x] that shape, measured. qb-qrender's 191 bodies hold 743 `fld` against
      333 `fstp`, 426 in-place arithmetic and 76 popping. What the gap is
      *not* is a store immediately reloaded: **0 sites** of `fstp [x]`
      followed by `fld [x]`, in qb-qrender and in the corpus, adjacent
      either way. BC does not write the obvious x87 peephole any more than
      it writes the obvious integer ones

- [ ] the one shape that is there, and it is thin. Allowing a gap, a cell
      stored by `fstp` and later read by `fld` with nothing writing it in
      between happens **36 times in qb-qrender** and 6 in the corpus, at a
      mean distance of 6.4 operations. 19 sit at loop depth 0, 16 at depth
      1, 1 at depth 2.

      Not built, and the reason is the ratio rather than the count: keeping
      a value on the x87 stack across six operations means scheduling the
      stack, since every `fld` between them rotates it -- the hardest
      transform in this project for 36 static sites in 26,290 instructions.
      `fpstack.py` is what it would be built on and exists.

      What would change the answer is a cycle measurement showing those 17
      in-loop sites are hot. Static depth is not that: `docs/roadmap.md`'s
      own PITSNAP note is about a busy-wait that scores two loops deep and
      runs twice

## Gates

Correctness is against the input. Byte-identity only ever checked the path
nothing transformed.

```
uv run pytest -m "not e2e"                     host suite
uv run pytest                                  adds DOSBox
uv run python tools/matrix.py                  19 programs x 12 configurations
uv run python tools/mutate.py                  deliberate breakages, each caught
uv run python tools/fuzzcheck.py --count 40    generated programs, BC as oracle
```

`matrix.py` and `fuzzcheck.py` both take `--no-absorb-calls`, which leaves
the arithmetic calls to the MIR tower instead of `calls.py`. That is not an
alternative configuration to keep green as a courtesy: it is the only way
the MIR emitter is reachable at all, since `calls.py` takes every site
before a body gets there. Both are green under it, all twelve
configurations.

`matrix.py` and `fuzzcheck.py` found every real bug this session. The host
suite was green for all of them.

A benchmark found the one they missed. `bench/fpbench.bas` printed
-2147483648 for every coordinate: `forward.py` read the base register of
`fld dword ptr [si]` as the load's destination, so two pushes of the same
address looked like a load and a redundant reload, and deleting the second
slid every x87 slot after it. Nothing in the suite could have caught it --
every other float program here is one operation deep, and `fuzzgen.py`'s
floats stay inside the exactly-representable integers. `suite/fpdeep.bas`
is the two-deep indexed shape, added so the corpus holds it now.

Widening the generator to cover it found a second one immediately. It made
two arrays and handed the widths out in the order INT, LNG, SNG, DBL, so a
float array had never once been generated; and `_ensure_valid_operand`
required a LONG among a node's operands, which quietly caught SINGLE and
DOUBLE too, so every float BinOp had its right operand replaced by a plain
variable. With one array of each width, a repeated operand, and that guard
narrowed to LONG, the first run turned up `simplify.py` deleting a round
trip whose register was overwritten between the pushes and the pop. That
one changed a real answer: `x MOD y MOD z` used the first divide's divisor
for the second.

**And the fuzz corpus is the only thing that has ever caught a
whole-segment bug.** Both of the ones that stood in the way of turning
emission on were invisible to the host suite and to all twelve
configurations, because the suite's programs do not contain the shapes: BC
writes `add ax,offset X` only for an array reached by adding its own
address into ax, and reaching `redundant()` with an accumulate needs the
memory map to hold that cell, which takes longer code than any suite
program. Neither has a fixture, so neither has a corpus test -- what guards
them is a test of the wiring in `layout` and a unit test on
`avail._preserved`.

Finding them needed one more tool than the gates: reducing a generated
program with the machine build as the oracle, which needs no authored
golden and shrank F004 from 173 lines to 31. Worth rebuilding as a script
if a third one turns up.

**Read the exit code, not the last line.** `mutate.py` gets this right: a
mutation whose pattern no longer matches is counted as survived and the run
returns 1, and its own docstring says so. What hid it was the invocation --
`uv run python tools/mutate.py | tail -5` reports `tail`'s status, so a
failing gate came back "exited with code 0" three times in one session while
printing `pattern appears 0 times` in plain sight.

The check that had lapsed was `segment-override-not-refused`, pointed at a
line in `lift.py` that was rewritten when an override became something
resolved rather than refused. The bug it existed to catch then shipped:
widening dropped the override off a far pointer and qb-qrender corrupted
itself.

Both lapsed mutations are repointed and the gate is 40 of 40. Pipe a gate
through anything and `set -o pipefail` first, or do not pipe it.

Known gap, and it predates this: on the three QuickBASIC 4.5
configurations one generated program in forty diverges between the
evaluator and BC's own build. `fuzzcheck.py` excludes those from the qbopt
comparison, so it costs coverage rather than correctness.

What it is not, measured on the pre-change generator's F001: an arithmetic
disagreement. Every label both printed carries the same value. The program
prints 8 lines where the evaluator expects 41 -- one contiguous run of
statements produces no output at all, and then execution resumes and runs
to DONE. Whatever 4.5 does there, the evaluator models VBDOS and PDS, which
both agree with it. Worth an interactive run under 4.5 to find; it has not
had one.

## Housekeeping

- [x] `docs/architecture.md` — brought up to date, including that the MIR
      tower emits now and that `rewrite.py` does not call it

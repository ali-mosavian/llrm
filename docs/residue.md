# Residue

What survives absorption and widening, on `bench/nbody.bas`'s rewritten object,
found by manually tracing register liveness through the full disassembly
rather than by running the pass's own tests. None of this is wrong -- every
number in `docs/numbers.md` is real -- but the rewritten object is 137 bytes
*larger* than BC's own (1447 against 1310, in the code this measurement
covers), and essentially all of that growth, plus more, is provable slack.

Measured 2026-08-29, `build/bench/v-g3/NBODY.OBJ` (base) against `NBODYQ.OBJ`
(opt), VBDOS `/G3`. Every count below was independently re-derived by hand
against the actual instruction stream, not assumed from a first pass -- an
earlier, script-driven attempt at this same catalog had a real bug (treating
`ax`/`dx` liveness as one joint unit instead of two independent registers,
which hid a genuine dead case) before this document was written.

**Re-measured 2026-08-30**, after this session's own comparison-absorption
correctness fix (`f2b6f05`) and G and H's own closure (below) both changed
what code sits next to what survives. `bench/nbody.bas`'s rewritten object
is, right now, **80 bytes *larger*** than BC's own (1440 against 1360, +5.9
per cent) -- not smaller, and not the 137/1447/1310 figures above, which
predate both fixes and are kept here only as history. The corpus-wide census
(`docs/numbers.md`) is still a real 19 per cent smaller in aggregate; this
one program currently regresses. B, D, E, F and I below are re-counted fresh
against the current object by `tools/residue_census.py` -- a mechanical walk
of `qbopt/ir.py`'s own node stream, cross-checked by hand against
`build/dump/NBODYQ/ir.txt` for 2-3 instances per pattern (noted per section)
-- not reread from the disassembly by eye a second time. Every remaining
instance below is additional, real slack on top of the +80: fixing all of it
would not by itself flip the sign on this file, because most of what grew it
was `f2b6f05`'s own necessary correctness cost.

## A -- `popped_into`'s recombination is an unconditional no-op

**15 instances.** `qbopt/calls.py`'s `popped_into()` pops a two-word argument
low-then-high, then does `push bx / push <lo16> / pop <target32>` to
"recombine" them -- but two consecutive 16-bit pushes already leave the pair
contiguous in dword layout on a 386; `pop e<target>` alone reads exactly that.
The four middle instructions read back the identical bytes they just wrote to
the identical stack address and push them again. The only real effect is
clobbering `bx`.

```
006E  push dx            BC's own operand push, kept
006F  push ax            BC's own operand push, kept
0070  pop ax             no-op
0071  pop bx             no-op (clobbers bx)
0072  push bx            no-op
0073  push ax            no-op
0074  pop eax            this alone would have sufficed at 0070
```

15 of 16 `consume()` sites in this object hit it; the sixteenth had both
operands already pushed as dwords and correctly emits just two pops. Root
cause: `popped_into()`'s own docstring reasons about the halves as needing
recombination, the way `restoring()` does in the opposite direction -- they do
not, because the stack already holds them combined.

**Fix**: trivial and strictly safer than today. Both branches of
`popped_into()` become `[Instruction.create_reg(Code.POP_R32, target)]`; the
undocumented assumption that clobbering `bx` is harmless disappears rather
than needing to keep being true. ~60 bytes.

## B -- restore, then re-push, then A's no-op: a round trip to nowhere

**3 instances**, wherever a chained call (Case B/D from the call-absorption
redesign) meets pattern A.

```
01EA  imul eax,ecx
01EE  push eax          calls.py's restoring()
01F0  pop ax
01F1  pop dx
01F2  push dx           BC's own operand push for the NEXT call
01F3  push ax
01F4  pop ax            calls.py's popped_into() -- pattern A
01F5  pop bx
01F6  push bx
01F7  push ax
01F8  pop eax           bit-for-bit what eax already was at 01EE
```

`eax` is exactly what it was ten instructions and twelve bytes earlier;
nothing in between reads `dx` or `bx`. This is possible because
`push eax / pop ax / pop dx` leaves `eax` itself completely intact --
`pop ax` writes back into `eax`'s own low half what `push eax` just read out
of it -- a fact worth stating on its own, since it is what makes several of
these fixes cheap: after every one of the 24 restores in this object, the
32-bit result is still sitting in the original register, whether or not
anything downstream knows it.

**Fix**: A alone shrinks this to 6 bytes. Removing the rest needs a peephole
over the *emitted* stream recognising "a restore immediately followed by
another absorption's own operand load" -- adjacency only, no dataflow, but a
new step, since `calls.py` codegen has no view past its own region and
`consume()` has no view before its own start.

**Re-measured 2026-08-30, still 3 instances, now 24 bytes** (`0x01d4`,
`0x0200`, `0x0337`; addresses moved, the shape and count did not).
`tools/residue_census.py` walks the node stream for a `Restore` immediately
followed by three `Opaque` nodes -- push hi, push lo, pop the 32-bit root,
all four contiguous in address and using exactly the restore's own pair's
registers -- which is this pattern with A already fixed: the old five-
instruction "A's no-op" collapsed to `pop eax` alone, so what is left is
purely the restore (4 bytes) plus that single, still-pointless round trip
(push+push+pop, 4 bytes) = 8 bytes/instance, all of it removable since
nothing between the restore and the pop ever reads `dx`/`bx` on its own.
Hand-verified against `build/dump/NBODYQ/ir.txt` at `0x0200` and `0x0337`:
both are `imul`/`idiv` results already correct in `eax`, restored to `dx:ax`
and immediately re-collapsed back into `eax` to feed the next absorbed call
or division, with the restore's own two-word push at `0x0337-0x033c`
resurfacing bit-for-bit as `pop eax`'s own input three instructions later.

## C -- squaring reloads the same address twice

**1 instance.** `apply_to()` builds its operand independently of `load_of()`,
so `x*x` loads `x` from memory twice instead of once.

```
0159  mov  eax,ds:[0x76]
015D  imul eax,ds:[0x76]      -- identical Addr: same fixup, same disp, no base
```

`imul eax,eax` is 4 bytes against 6, and drops a memory read. `Addr` is
already a frozen dataclass with structural equality; this is one `if` in
`absorb()`. Only one instance in this object, but `x*x` is common in
fixed-point code generally.

## D -- immediate-operand pair ALU is missing from `lift.py`'s own table

**3 instances**, and unlike everything above, this one has nothing to do with
absorbed calls at all. `lift.PAIRED` only lists the `r16,rm16` forms;
`add ax,imm16` / `adc dx,imm8` is a different opcode pair BC also emits, and
`classify()` returns `None` for it, invalidating both tracked pairs on the
spot.

```
0189  add ax,0          ; 05 00 00
018C  adc dx,4          ; 83 D2 04     -- together, += 0x40000
```

`032C add ax,1 / 032F adc dx,0` proves this is independent of the call-barrier
problem below: it is preceded by a perfectly liftable load pair
(`0325 mov ax,[0x92]` / `0328 mov dx,[0x94]`), and `tools/rewrite --report`
already names the casualty --
`02E3-02EA taken=False widening it grows 7 bytes to 8` -- because losing the
add/adc truncates the region to a lone load, and a load plus a restore is
bigger than what BC wrote.

**Fix**: add the immediate forms to `PAIRED`, give `Value` an immediate
operand alongside `mem`, and combine the two halves' immediates as
`(high<<16)|low`. Local to `lift.py`; no cross-region reasoning needed.

**Re-measured 2026-08-30, still 3 instances, now 18 bytes local** (`0x017e`,
`0x01a0`, `0x02ea`). `tools/residue_census.py` scans for two adjacent
`Opaque` nodes whose codes are the immediate-operand mirror of `lift.PAIRED`
(`ADD_AX_IMM16`/`ADC_*_IMM8`/`ADC_*_IMM16` and the `AND`/`OR`/`XOR`/`SUB`
siblings), same pair, opposite halves, byte-contiguous -- confirmed against
the dump by hand at `0x017e` (`add ax,0` / `adc dx,4`) and `0x02ea` (`add
ax,1` / `adc dx,0`). The 18 bytes is the two immediate instructions' own
length against one widened `add/sub/and/or/xor r32,imm32`; it is a floor, not
the real payoff -- `0x02ea`'s own pair is exactly what truncates
`0x02e3-0x02ea`'s region to a lone load today (see E, below, for why that
refusal is D's fault and not counted there), so the real saving once fixed
includes whatever the reunited load+alu+store region gains on top of this,
unmeasured here. Independently found by a parallel line-by-line read of this
same object, same three addresses, same count -- the strongest cross-check
of any pattern in this document.

## E -- one interleaved instruction splits a long expression that lift.py can't step over

**2 confirmed** by `tools/rewrite --report`'s own refusals, the shape is
likely commoner than the count suggests. `regions()` and `match()` both
require strict address contiguity; BC routinely drops address arithmetic or a
spill between the two halves of what is, semantically, one long expression.

```
0122  mov ax,[si+0x6]        liftable LOAD pair
0126  mov dx,[si+0x8]
012A  mov di,ds:[0x96]       unrelated: the OTHER array's index
012E  shl di,2
0131  sub ax,[di+0x6]        liftable ALU pair -- but live[0] was cleared at 012A
0135  sbb dx,[di+0x8]
0139  mov ds:[0x76],ax       liftable STORE pair, likewise cleared
013C  mov ds:[0x78],dx
```

`0113-011B taken=False  widening it grows 8 bytes to 9` is exactly this: the
planner saw only the isolated 8-byte load pair, and a widened load plus its
restore (9 bytes) loses to leaving it alone. The structurally identical
statement immediately below it, with no interleaving, *was* widened. The same
shape defeats call absorption too: base `04D4 push dx / 04D5 push ax /
04D6 call B$MUI4` is contiguous, but its other operand sits at `0469` with a
spill in between, so `match()` refuses it and it falls through to `consume()`
-- which is most of why pattern A/B exist at all (16 of this object's 21
absorbed call sites went through `consume()`, not `match()`).

**Fix**: the large one. Needs either real def/use analysis proving an
interleaved instruction is safe to move across the region (`si`/`di`/flags),
or a region shape that tolerates a hole. Everything else in this document is
local; this one is architectural.

**Re-measured 2026-08-30: 1 confirmed genuine instance, not 2.**
`tools/rewrite --report` still shows two regions refused with a "widens N
bytes to M" reason (`0x0113-0x011b` and `0x02e3-0x02ea`), which is what the
original count of 2 came from -- but `0x02e3-0x02ea` is D's own doing, not
E's: that region is already fully contiguous (load pair, immediate add/adc
pair, store pair, no gap at all), and it only shrinks to a lone load because
`classify()` cannot see the add/adc as a pair, exactly D's own worked
example. Counting it under both patterns would double the same byte cost.
The one real interleave left is `0x0113-0x011b` (worked example above,
addresses unchanged from the original measurement) -- confirmed by hand
against the dump: `mov ax,[si]` / `mov dx,[si]` (a clean load pair) is
followed by `mov di,ds:[0]` / `shl di,2` (address arithmetic for the *other*
array, writing only `edi`) before the `sub`/`sbb`/store triple that was
always going to consume the load. `tools/residue_census.py` cross-references
its own D findings against every "grows" refusal to make this split
mechanical rather than another by-hand judgement call. A parallel,
independent line-by-line read of this object reached the same conclusion --
one real instance, and specifically named the interleaving instructions as
writing only a third, non-conflicting register, which is what makes this
refusal fixable by something short of full dependence analysis: proving
`di`/`edi` non-interference for this one shape, not a general mover.

## F -- sign-extension is invisible to both passes

**~9 instances** (every `cwd` in the object: `006D`, `009F`, `00F2`, `04B9`,
`04D4`, `0507`, `053A`, `0565`, `0575`). `mov ax,<r16> / cwd` -- INTEGER
widened to LONG -- is not a value `lift.py` tracks and not an operand kind
`calls.py` recognises. `widened_constant_at()` already handles the *immediate*
form of this idiom (`mov ax,imm16/cwd/push dx/push ax`); the register-sourced
form falls straight through to `consume()`.

```
04CC  mov [bp-20h],dx      valid STORE pair, but live[0] already cleared
04CF  mov [bp-22h],ax         by 04B7 mov ax,bx / 04B9 cwd just above it
04D2  mov ax,bx
04D4  cwd
04D5  push dx
04D6  push ax
04D7  ... pattern A's five-instruction dance
```

`movsx eax,bx` is one instruction where `mov ax,bx / cwd` is two, and would
let this become a direct register load with no stack traffic at all.

**Fix**: medium, but contained -- a `MOVSX` value in `lift.py`, and a
register-sourced operand kind in `calls.py`. Both are new node types, not new
analyses.

**Re-measured 2026-08-30, still 9 `cwd` instances**, same count as the stale
table, addresses moved. `tools/residue_census.py` flags every `Opaque cwd`
node except the one shape `calls.widened_constant_at()` already owns (`mov
ax,imm16 / cwd / push dx / push ax`, all four contiguous). The 9 split three
ways, all invisible to `lift.py` for the same underlying reason (no `MOVSX`
value) but with different immediate sources: 4 are `mov ax,bx / cwd` --
residue's own original worked shape, hand-verified at `0x047a` -- 2 are
`mov ax,<r16>/[mem] / cwd` where the mov is itself a recognised `Long` LOAD
node lift.py already tracks, hand-verified at `0x0514` (`mov ax,[bp-18h]` is
a `Long ld`, but the `cwd` right after still isn't consumed by anything); 2
are an ALU result sign-extended in place (`sub ax,0Fh / cwd`, `0x0071`); and
1 (`0x00eb`, `mov ax,1 / cwd`) is a constant whose sign-extension IS the
`widened_constant_at()` shape's inputs but not its trigger, because a `jmp`
sits between the `cwd` and the `push dx/push ax` that shape requires
contiguous -- it jumps into a shared tail instead, landing mid-way through
one of D's own store pairs. A parallel, independent line-by-line read of
this object counted 6 for what it named this pattern -- consistent with
counting only the 4 register-mov and 2 recognised-LOAD cases above and
treating the 2 ALU-result and 1 jump-diverted constant cases as distinct (or
not counting them at all); this document keeps the broader 9, since all nine
share the identical root cause (no value `lift.py` can hand a `cwd` node),
but the narrower 6 is the more conservative number if only the literal
`mov ax,<r16>/cwd` shape from the original worked example is wanted.

## G, H -- the absorbed-call barrier: `lift.py` can't see past a restore

**G, store not collapsed, 4 instances** (`007C`, `00AE`, `0163`, `01CA`).
**H, ALU not widened, 11 instances** (`017F`, `01AB`, `0201`, `0237`, `0291`,
`02BC`, `037B`, `04E3`, `0516`, `0549`, `0584`). The largest category, and the
one with the biggest single payoff, because the failure cascades through the
rest of the statement:

```
028E  idiv ecx
0291  push eax
0293  pop ax
0294  pop dx
0295  sub ax,[si+0x3e]
0299  sbb dx,[si+0x40]
029D  neg ax                the 8086 32-bit negate idiom --
029F  adc dx,0              lift.py already has this (NEGATE/negate_at,
02A2  neg dx                whose own comment calls missing it "expensive
02A4  mov [si+0x3e],ax      out of all proportion") -- unreachable here only
02A8  mov [si+0x40],dx      because live[pair] was already cleared
```

27 bytes, 11 instructions. Widened: `sub eax,[si+0x3e] / neg eax /
mov [si+0x3e],eax` -- 13 bytes, 3 instructions.

One root cause for both G and H: `calls.py`'s restore and `lift.py`'s value
graph are two passes that never talk. `absorb()`/`consume()` emit bytes;
`lift()` re-reads those bytes as an opaque `pop` it doesn't recognise and
drops both tracked pairs, per its own documented policy ("anything not
recognised invalidates both pairs, because an instruction this does not
understand may write either of them" -- correct as a default, wrong here
specifically, because the actual value is knowable). Pattern B already
established the fact that makes this fixable: `push e?x / pop ?x / pop ?x`
leaves the 32-bit register intact, so the value the lifter would need to
resume tracking is right there.

**Fix**: two sub-pieces. Recognising a restore as a value-defining instruction
is local to `lift.py`'s `classify()`/`lift()` -- but `rewrite.plan()` runs
`lift()` over the *original* code before absorption happens, so this needs
either a second lift pass over the already-emitted stream, or the absorption
result fed back in as a synthetic `Value`. That is a pipeline change, not a
new kind of analysis. Actually dropping the now-provably-unneeded restore
after that is pattern I's fix, below.

**Fixed, 2026-08-30.** The counts above were stale by the time this was
built: `f2b6f05`'s compare-absorption fix changed which restore-adjacent
shapes exist in this object, and a re-measurement against the corrected
`NBODYQ.OBJ` -- a script against `ir.py`'s own node stream, walking for
`ir.Restore` and looking at what immediately follows for the same pair,
rather than manual disassembly reading -- found **12** instances (2 G, 10 H),
not 15. The fix took the second of the two sub-pieces above: the absorption
result fed back in as a synthetic `Value`. `qbopt/lift.py` gains an `Op.CALL`
value (its bytes are `calls.py`'s own already-assembled `Emitted`, verbatim,
not something `instruction()`/`Encoder` builds) and a `tail()` function that
seeds pair 0 with it and re-runs `lift()`'s own pairing rules
(`_negate_step`/`_pair_step`, extracted unchanged so `tests/test_lift.py` is
the regression proof nothing about ordinary widening moved) -- so the NEGATE
idiom in the worked example above is covered for free. `calls.py`'s
`absorb()`/`consume()`/`dividing()`/`fix_multiply()` gain a `restore: bool =
True` parameter, so a call folded into a wider region drops its own trailing
restore rather than putting the high half back only to immediately re-derive
it from eax. `qbopt/rewrite.py`'s new `tail_widened_calls()` composes the
two -- call plus widened tail -- into one ordinary `Edit`, going through the
exact same `needed()`/`refuse()`/`emit_region()` machinery (and the same
DIVERGENT flags gate) an ordinary widening region already does; anything
that does not work out falls straight through to the unmodified, narrow-
boundary standalone `absorb()` this project already had. On `bench/nbody.bas`
specifically: 12 call sites folded, 59 bytes saved over what standalone
absorption plus an un-widened tail would have produced. Corpus-wide
(`fixtures/omf`, 110 much smaller, more varied programs), 19 instances, 57
bytes -- see `docs/numbers.md`.

## I -- restores that are simply dead

**6 instances** (`00EB`, `0151`, `0155`, `027D`, `0311`, `0315`) -- **all from
`lift.py`'s own `restored_pairs()`, none from `calls.py`**. Zero of the 17
restores `calls.py` emits in this object are dead; every one of the pass's
own restores is.

```
0140  mov ecx,[si+0x22]
0145  sub ecx,[di+0x22]
014A  mov eax,ecx
014D  mov ds:[0x7a],eax
0151  push eax / pop ax / pop dx      pair 0, dead
0155  push ecx / pop cx / pop bx      pair 1, dead -- and doubly pointless,
                                       since 014A already means both pairs
                                       hold the same value
```

4 of 6 are killed later in the same block (`00EB`, `0151`, `0155`, `027D`); 2
(`0311`, `0315`) need one block of lookahead through a shared successor,
where `dx`/`cx`/`bx` are each killed before being read on every path.

**Fix**: the cheapest of the real fixes, because the machinery already
exists. `qbopt/flags.py` is a complete iterative backward liveness analysis
over the block graph (`reads()`, `writes()`, `live_in()`, `live_after()`),
and `rewrite.plan()` already computes `flags_after(blocks, live, at, end)` and
threads it into `emit_region` -- the call site and the plumbing are in place.
Duplicating it for a 4-bit `{ax, dx, cx, bx}` set instead of the flag bits is
close to mechanical. The one new judgement call is what a `call far` does to
this liveness: it must be treated as reading and writing all four (BC does
pass arguments in `ax`, e.g. `039B push ax / 039C call B$STI2`) -- checked,
and all 6 dead cases here survive that conservative model; no live case is
ever killed by a call in between.

**Re-measured 2026-08-30: 8 confirmed instances, 32 bytes** (`0x00e4`,
`0x014a`, `0x01ec`, `0x0218`, `0x024f`, `0x0270`, `0x0291`, `0x02cf`, all
pair 0 -- `bx`'s own restores, pair 1, are all still live). `tools/
residue_census.py` reimplements this section's own planned fix: a real
`live_in()`/`live_after()` over `qbopt.blocks`' own `Block`/`succ` graph, one
register at a time, `call far` treated as reading and writing conservatively
as described above. It deliberately does *not* reuse `ir.py`'s own
`Effects.defs`/`.uses` for this -- those root every sub-register write to its
32-bit parent and count it as also a *use* of the root (`ir.py`'s own,
correct-for-its-purpose docstring: "a write to a sub-register... leaves the
top half... stale", so the root has to be treated as still-needed). Doing
that here would make a plain `pop dx` look like a read of the OLD `dx`, which
is the exact "`ax`/`dx` as one joint unit" bug this document's own intro
already warns a prior script got wrong -- so this script checks the literal
`dx`/`bx` (and `edx`/`ebx`) iced reports, not the rooted version. Hand-
verified against the dump at `0x02cf`: the restore is followed by a second,
adjacent pair-1 restore (irrelevant to `dx`), then a `Long` load/inc/store on
`ax` alone and a `cmp`/branch back into the loop -- `dx` is never touched.

A parallel, independent line-by-line read of this object counted 12 for the
same pattern (10 provable by intra-block/trivial-CFG liveness, 2 more
needing "a far call doesn't read `dx`/`bx`" as an established fact). The 4-
instance gap is real and unresolved: this script's cross-block liveness
treats every `call far` as conservatively reading `dx`/`bx`, per the
judgement call this section's own fix plan already flagged, which can turn a
truly-dead restore "live" the moment any call sits between it and the point
where `dx`/`bx` is next overwritten on some path. The manual audit's 2
"needs the fact" cases are exactly that situation; the other 2 of the gap are
unaccounted for and worth a second look. Establishing which of BC's own
runtime routines never touch `dx`/`bx` (the way this session already
established it for `B$CPI4`'s flags-only contract) would very likely close
some or all of the gap, but that fact has not been checked here -- 8/32 is
what this document treats as confirmed; up to 12/48 is plausible pending
that check.

## Inherited from BC, not this pass's doing

Noted for completeness, not proposed as work: **`mov es,[0x0]` reloaded four
times** (`04AE`, `04C3`, `04F6`, `0529`, byte-identical, nothing writes `es`
between them); **the same array index recomputed** (`mov si,[0x96]/shl si,2`
at `0080`, `0258`, `03CA`, and the `di` counterpart at `012A`); **a
just-stored value reloaded** at `0281` because `lift.py` inherits BC's own
register-pair choice and had no freedom to keep it live in a different pair;
**spills that no longer protect anything** (`0171`/`0174` and three more)
since the far call they once had to survive is gone and `imul r32,rm32`
doesn't even write `edx` -- a register would replace six instructions per
site. All four would need real cross-region register allocation to fix, which
is the same architectural gate the call-absorption redesign already
concluded on: memory disambiguation this project does not have.

## A parallel, independent catalog

A separate, line-by-line human read of `build/dump/NBODYQ/ir.txt` ran
alongside this document's own 2026-08-30 re-measurement and named its own
11 patterns (F1-F11) rather than reusing B/D/E/F/I's letters. Where the
correspondence is clear from its own description, it is noted inline above:
F4 is D (same 3 addresses, independently arrived at); F5 is E (same
1-instance interleave, same root cause); F7 is F's narrower, 6-instance
reading. F1 (its dead-restore count of 12 against this document's confirmed
8) is discussed under I, above, as an open gap rather than a resolved match.
F2, F3/F3b, F6, F8-F11 describe shapes this document's own B/D/E/F/I letters
were never scoped to cover (store-pair merging blocked by a non-conflicting
interleaved write, a decoder gap for a zero-displacement store form,
operand-fold fallback when a push and its call are not adjacent, and several
smaller independent misses) -- real, and not yet folded into this catalog.
A full rename of this document's remaining letters to that audit's own
finer-grained numbering is worth doing, but needs that audit's own detailed,
address-by-address findings as the source rather than the short summary this
re-measurement had to work from; not attempted here.

## Priority

Re-measured 2026-08-30 against the current object (`tools/residue_census.py`;
see each pattern's own "Re-measured" paragraph above for method and
hand-verification). Counts and bytes below supersede the stale ones this
table originally shipped with.

| pattern | count | bytes | fix scope |
|---|---|---|---|
| A -- stack no-op in `popped_into` | 15 | ~60 | **fixed**, 2026-08-30 |
| I -- dead lift.py restores | 8 confirmed (up to 12) | 32 (up to 48) | reuse `flags.py`'s own machinery |
| B -- restore/re-push identity | 3 | 24 | peephole, adjacency only |
| D -- immediate pair ALU missing | 3 | 18 local (floor, real payoff larger) | table entries + one `Value` field |
| C -- redundant self-multiply reload | 1 | 2 | **fixed**, 2026-08-30 |
| G+H -- restore blocks widening | 12 (was 15, stale) | 59 | **fixed**, 2026-08-30 |
| F -- sign-extension invisible | 9 (6 on the narrowest reading) | not locally computable -- architectural | new value + operand kind |
| E -- interleaved instruction splits a region | 1 (was "2 confirmed", stale -- the other was D's own doing) | not locally computable -- the large one | dependence analysis + motion |

A, C and G+H are fixed. A and C: `qbopt/calls.py`'s `popped_into()` and
`absorb()`; corpus-wide, not just this object, they took the static census
from 17414 to 16942 bytes. G+H: `qbopt/lift.py`'s `tail()`, `qbopt/calls.py`'s
`restore=` parameter, and `qbopt/rewrite.py`'s `tail_widened_calls()`; static
census unchanged in region *count* (commit 3 widens 19 existing call regions,
corpus-wide, rather than adding new ones) but 57 bytes better, net, than
before it. See `docs/numbers.md` for both.

Despite G+H's own closure, `bench/nbody.bas`'s rewritten object is currently
**80 bytes larger** than BC's own, not smaller -- see the note at the top of
this document. I and B together are a real, locally-computable 56 bytes (up
to 80 if I's higher bound holds) without touching this project's
architecture; D adds a further, not-yet-fully-measured amount once its own
region-level payoff is counted. F and E remain the architectural ones still
open, and neither has a locally-computable byte figure -- both need the
actual fix built before their real payoff is knowable, not a bigger
regex.

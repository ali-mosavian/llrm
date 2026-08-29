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

## Priority

| pattern | count | bytes | fix scope |
|---|---|---|---|
| A -- stack no-op in `popped_into` | 15 | ~60 | **fixed**, 2026-08-30 |
| I -- dead lift.py restores | 6 | ~24 | reuse `flags.py`'s own machinery |
| B -- restore/re-push identity | 3 | ~24 | peephole, adjacency only |
| D -- immediate pair ALU missing | 3 | ~15 | table entries + one `Value` field |
| C -- redundant self-multiply reload | 1 | 2 | **fixed**, 2026-08-30 |
| G+H -- restore blocks widening | 15 | ~107 | feed absorption back into `lift()` |
| F -- sign-extension invisible | ~9 | ~30 | new value + operand kind |
| E -- interleaved instruction splits a region | 2 | ~15 | dependence analysis + motion -- the large one |

A and C are fixed (`qbopt/calls.py`'s `popped_into()` and `absorb()`);
corpus-wide, not just this object, they took the static census from 17414 to
16942 bytes -- see `docs/numbers.md`. I, B, D together are roughly 65 bytes
more, on their own, without touching this project's architecture. G+H, F and
E remain the architectural ones.

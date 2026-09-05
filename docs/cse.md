# What CSE needs, measured on lngmix

`s = s + v \ 7 + v MOD 7` in a loop over operands that never change. 1,844
against a target of 210, the worst on the board with lngmxx.

x86 yields quotient and remainder from one `idiv`, so this is one divide.
We emit two, and neither leaves the loop.

## The two divides are the same computation, and MIR cannot see it

```
  0x004e  load     v1_4        <- [v]
  0x0052  copy     v2_3        <- 7
  0x0058  convert  v3_3        <- v1_4
  0x005a  div      v1_5, v3_4  <- v3_3, v1_4, v2_3     quotient kept
  ...
  0x0079  div      v1_7, v3_7  <- v3_6, v1_6, v2_4     remainder kept
```

`v1_6` and `v2_4` are not `v1_4` and `v2_3`, so no value numbering can
match them. They are the same numbers, pushed and popped:

```
  0x0061  arg #0          st [sp-0x2]
  0x0063  arg #7          st [sp-0x4]
  0x0065  arg [v_hi]      st [sp-0x6]
  0x0069  arg [v_lo]      st [sp-0x8]
  0x0073  result v1_6:4   ld [sp-0x8]     the pair at sp-8 and sp-6 -- v
  0x0075  result v2_4:4   ld [sp-0x4]     the pair at sp-4 and sp-2 -- 7
```

The slots make the identity exact, so a pass could recover it. But the
round trip is not in the program: it is what `calls.py` leaves behind when
the **machine arm** absorbs `B$DVI4` and `B$RMI4` into `idiv`. It emits the
arguments as pushes and pops them back, because at that layer there are no
values to hand the instruction.

## So the order is phase D first

Absorption at the raise -- the plan's phase D -- emits `DIVIDE` over the
argument *values* and the round trip never exists. Then the two divides are
the same key by construction, cse matches them, and the hoist takes the
whole thing out of the loop, which it currently refuses because
`_invariant_run` will not touch a loop holding a call.

Writing a stack-propagation pass instead would work around a round trip
that phase D deletes. lngmix and lngmxx are 8.8x and 8.9x and both are this
program; fpcse and fpcsex at 3.3x and 3.4x are cse without the round trip
and would come first if a smaller step is wanted.

## What phase D runs into

Measured: 1,211 absorbable sites in the corpus, 1,037 of them with operands
the raise could name outright -- 2,011 at a static address, 63 immediates.
Only 174 need `consume`, where something is on the stack and nothing names
it. So the recognition side is easy and mostly already written:
`calls.sites()` finds them and `calls._deleting()` already builds the
machine sequence each becomes.

The lowering side is where it stops. One MIR `DIVIDE` over two memory
operands becomes four instructions --

```
  mov eax,[a]      relocated
  mov ecx,[b]      relocated
  cdq
  idiv ecx
```

-- and **two of them carry a fixup**. `select.Emitted` reports one
relocated field, because until now one operation has been one instruction
and an instruction relocates at most one. `select.restore` already emits
three instructions and gets away with it by having no fixup at all.

So phase D needs `Emitted` to carry a fixup per instruction, and
`layout._field_in` and `relocate.py` to place more than one. That is a
data-structure change across three modules and it is the real content of
the phase -- not the recognition, which is done.

The machine arm already does this: its edits carry their own fixup list.
What phase D moves is that capability from `calls.py` into `select.py`,
where the rest of emission lives.

## Where the flip stands

`absorb_calls=False` -- the raise absorbing instead of the machine arm --
is **correct**: 40 programs on 12 configurations, all pass. It costs 647
bytes over the corpus (642,201 against 641,554), and it is not the default
yet, because the byte cost is real and the gain is not collected until cse
and the hoist use the shape.

lngmix now holds one of its two divides as an operation over values:

```
  loop 0x8f: 19 ops, calls ['0x71']
    0x004e  div  [seg:5+0x6]:4, 7
```

The other is still a call. `calls.sites` classified it `consume` -- four
pushes it could not name -- because BC put the first divide's result stores
between the pushes and the call, and `match()`'s backward scan only sees a
push immediately, contiguously before one.

The information is there: `site.consume` holds those four push
instructions and `calls.static_at` is what classifies one. Naming them is
the next step, and it is what makes both divides one key.

## Naming the pushes is not the blocker

Tried it: classify a `consume` frame's pushes with the same rules
`one_operand` uses, and check that nothing between the first push and the
call writes a byte an operand reads. It works -- both of lngmix's sites
come back named, its loop holds no call at all, and the two operations are

```
  0x004e  div  [seg:5+0x6]:4, 7
  0x005f  rem  [seg:5+0x6]:4, 7
```

which is one key twice, exactly what cse wants.

**And the programs are wrong on eleven of twelve configurations.**
Absorption *deletes* the region from the first push to the call -- that is
what makes it smaller than what BC wrote, four bytes of stack traffic per
operand going with it -- so anything else in that region goes too. What
sits in lngmix's is the previous divide's result stores.

Adding the region-is-clean test back makes it safe and makes it useless:
1,037 named and 174 consume, the same two numbers as before, because every
site whose region holds only its own pushes is one `match()` already finds.

## So the step is to move the stores out

They do not depend on the pushes -- they store the *previous* divide's
results -- so they can go before them, and then the region is clean and
`match()` finds the site on the next round. `rewrite` already iterates to a
fixed point, so one pass that sinks an independent operation out of a push
run is enough.

That is a MIR pass over values: an operation whose operands nothing in the
run defines may move ahead of the run. It is `place` -- the pass retired
earlier for buying nothing and asking `origin` which operations touched the
same register -- rebuilt to answer the question that has a use.

## Done: both divides fold

`place` moves an operation out of a call's push run when nothing in the run
defines what it reads and neither touches the other's memory. lngmix's
second run held the previous divide's two result stores; they go ahead of
it, `match()` finds a contiguous site on the next round, and the raise
absorbs it.

    round 0  952b   B$DVI4 pushed, B$RMI4 consume
    round 1  913b   B$RMI4 pushed
    round 2  909b   none

The pass alone did nothing. `layout.rebuild` sorted its op list on `op.at`
before emitting, so a reordered list came back in the original order and
every transform that moves an op was silently undone -- which is why the
old `place` "bought nothing measured". It now sorts on the lowest address
still ahead of an op in its own list, breaking ties by list position: the
old address sort exactly, on every program no pass reorders, and the new
order where one does. Bodies interleave by address -- a procedure sits
inside the main body's span -- so the sort still has to be global.

Both divides are now one operation over the same two operands. That is the
one key twice cse was waiting for.

## The pass

`transform.subexpressions()`. Value numbering, not a syntactic match: a name
is replaced by what it stands for as the walk reaches it, so a copy counts
as its source and an operation already folded counts as the one it folded
into. lngmix needs both, three links deep -- BC reloads `v`, reconverts it
and rebuilds the 7, so the second divide names none of the first's values.
Copy propagation is part of the question rather than a pass before it.

Three guards, each bought with a failure:

- **Nothing narrow.** A half of a long is `Held(value, 2)` and so is the
  other half, so two operations on opposite halves compare equal on the
  value they name. nots printed `NOTOR= 26390415` for `-271601777` -- right
  low word, wrong high one. Nothing in MIR says which half, so an operand
  narrower than its value is refused rather than told apart.
- **Only what its readers can find.** A value BC computed into a different
  place is the same value, and folding it is sound in MIR -- but a
  surviving reader then wants it where it no longer is, and only the
  allocator can arrange that. chain refused with `mov is not one select.py
  can emit`. A fold nothing outside the fold reads costs no register, which
  is what lets lngmix's convert go.
- **No flags a survivor reads.** In MIR they are an ordinary value; in the
  machine they are one register everything in between has written.

Byte accounting is whole-body (`_reclaimed`), not per block. The raise gives
every operation folded out of one runtime call the same `at` -- the site's
first push -- so `at` says nothing about which bytes an operation stands
for, and the operation before it in its own block is routinely elsewhere.
`covers` is the fact.

**Measured: 40 bytes, on lngmix-q-evt and lngmxx-q-evt.** The three
operations come out of lngmix-p-g2's MIR and the emitted segment is the
same 909 bytes, so something between the pass and the bytes puts them back.
That is the next thing to find, and it is worth more than the pass is.

## Why the bytes did not move: the body was thrown away

`layout.rebuild` colours every body, and **a body the allocator refuses is
emitted as it was raised** -- not as the passes left it. The reason is
sound: every operand resolves through the assignment, so without one each
operation would be written with the register BC had, while a pass has
already moved the operations that made that true.

lngmix's main body is one of them:

    v3_3 and a value it interferes with are both pinned to edx

So cse ran, `place` ran, the hoist ran, and none of it reached the bytes.
**103 of 487 objects have at least one body in this state**, and the
reasons are two:

    27  two values pinned to the same register interfere (eax, edx)
    76  a value interferes with every register at once

Measuring a pass on a program in that set measures nothing, which is the
second rule with a new instrument to distrust. lngmix at 8.8x is not
waiting on cse or on licm. It is waiting on the allocator, like everything
else on the roadmap -- and now with a name for what it refuses.

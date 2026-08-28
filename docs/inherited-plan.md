# Plan

Ordered. See README.md for the goal and the pipeline.

## Checklist

Every compiler, because they do not emit the same code and a pass that is
green on one may not reach the others at all. `y` means done and covered by
a test; `-` means not applicable; anything else is the state.

### Compiler options

Which switches each BC accepts, established by putting every one of them to
every compiler and reading the rejection. QB 4.5's options are in the
printed manual and in none of its three `.HLP` files -- `QB45QCK`, `QB45ADVR`
and `QB45ENER` decode with `HELPMAKE /D /T` but document the `QB.EXE`
environment, not the compiler. It also rejects differently: `Option unknown:
/Q`, where the other two say `Ignored unknown command line option`. Reading
for the wrong string reports every switch as accepted.

| | VBDOS | PDS 7.1 | QB 4.5 | matters because |
|---|---|---|---|---|
| `/G3` | yes | no | no | 386 codegen: one 32-bit argument push |
| `/G2` | yes | yes | no | 286 codegen: two word pushes |
| `/O` | yes | yes | yes | no BRUN; does not change the long shapes |
| `/Ot` | no | yes | no | no effect on them either |
| `/V` `/W` | yes | yes | yes | event polling: a call between statements |
| `/D` | yes | yes | yes | overflow checks; see below |
| `/Es` `/Ib` `/Ie` `/Ii` | yes | yes | no | |
| `/Fs` `/Lp` `/Lr` | no | yes | no | |
| `/A` `/Ah` `/C:n` `/E` `/FPa` `/FPi` `/MBF` `/R` `/S` `/T` `/X` `/Zd` `/Zi` | yes | yes | yes | |

There is no single optimisation level to turn up. The axes that change what
the pass sees are the codegen pair, which decides the push shape `qbo$mul`
absorbs, and `/V` `/W`.

### Configurations swept

`tools/qbe/matrix.sh` runs the whole suite over each. All twelve pass.

| | VBDOS | PDS 7.1 | QB 4.5 |
|---|---|---|---|
| shipping | `/O /FPi /R /G3 /E` | `/O /FPi /G2` | `/O /FPi` |
| 286 codegen | `/O /FPi /R /G2 /E` | - | - |
| no `/O` | `/FPi /R /G3 /E` | `/FPi /G2` | `/FPi` |
| no codegen switch | `/FPi /R /E` | - | - |
| `/Ot` | - | `/O /FPi /G2 /Ot` | - |
| event polling | `/O /FPi /R /G3 /E /V /W` | `/O /FPi /G2 /V /W` | `/O /FPi /V /W` |

Event polling earns its place. It puts a call between statements, which
fragments the blocks -- 6 regions become 32 over the same code -- and regions
the size check used to refuse are then taken. That is what exposed the
`qn$srcp` bug, where an absorbed multiply declared its high half dead and the
spill after it read a `dx` the widened form never wrote.

`/D` is compatible, checked with values that do not overflow, but is not in
the sweep: `src/test/qbeopt.bas` multiplies 305419896 by 252645135 on
purpose and `/D` makes that fatal before any comparison runs. Note that an
absorbed multiply drops the overflow check either way -- `imul` does not
trap -- which is the trade `inc/qbext.bi` already documents for `qbeBoost`.

### Operators on a long

Nested by compiler, then by the switches that change the answer. Only the
codegen pair does: it decides whether a long argument is pushed as one dword
or two words, and the two shapes leave different room for the inline form.
`/O`, `/Ot`, `/V`, `/W` and `/D` change none of these cells, which is what
`tools/qbe/matrix.sh` checks.

**VBDOS** -- takes both codegen switches, so it is the one compiler where a
single source can produce either shape.

| | what BC emits | `/G3` dword push | `/G2` word push |
|---|---|---|---|
| `+` `-` | add/adc, sub/sbb pairs | y | y |
| `AND` `OR` `XOR` | matching pairs | y | y |
| `*` | push, push, call `B$MUI4`, 15 or 21 bytes | y | y |
| `\` | push, push, call `B$DVI4` | y | y |
| `MOD` | push, push, call `B$RMI4` | fits rarely | y |
| unary `-` | `neg` / `adc hi,0` / `neg hi` | y | y |
| unary `-` | `xor ax,ax` / `cwd` / sub pair | no | no |
| `NOT` | `not ax` / `not dx` | no | no |
| `EQV` | xor pair, then not pair | half | half |
| `IMP` | not pair, then or pair | half | half |
| `=` `<>` `<` `>` `<=` `>=` | push, push, call `B$CPI4` | no | no |

`MOD` is the cell the shape decides. It needs `mov eax,edx` after the `idiv`,
which does not fit fifteen bytes once a fixup is charged, and does fit
twenty-one.

The `/G2` column is read off two measurements rather than assumed: VBDOS
`/G2` and PDS `/G2` produce byte-for-byte identical suite numbers -- 6
regions, 5 taken, 647 bytes to 409 -- where VBDOS `/G3` produces 629 to 416,
so `/G2` is emitting the word-push shape; and `src/test/qbeunit.bas` pokes
that 21-byte form at the pass directly and asserts all three of `*`, `\` and
`MOD` are absorbed from it.

**PDS 7.1** -- `/G3` rejected, so the word push is the only shape.

| | `/G2` | `/Ot` | neither |
|---|---|---|---|
| all of the above | as VBDOS `/G2` | same | same |
| `MOD` | y | y | y |

**QB 4.5** -- no codegen switch at all; rejects `/G2` and `/G3` both.

| | default |
|---|---|
| all of the above | as VBDOS `/G2` |
| `MOD` | y |

### Strength reduction

None of these are reached on any compiler or any switch, for one reason: a
constant operand is pushed as an immediate -- `66 6A nn` or `66 68 nnnnnnnn`
-- which is not the memory push the absorption matches.

| | | every compiler, every switch |
|---|---|---|
| `* 2^n` | `shl eax,n` | no |
| `* k` | `imul eax,k` | no |
| `\ 2^n` | `cdq`/`shr`/`add`/`sar`, 17 bytes | no; will not fit the dword shape |
| `MOD 2^n` | | no |

The multiply forms are comfortably smaller than what they replace; the divide
form is seventeen bytes against thirteen and does not fit the dword-push
shape at all.

### Addressing and shapes

**Every compiler, every switch** -- these come from how BC addresses a long,
not from any option.

| | VBDOS | PDS 7.1 | QB 4.5 |
|---|---|---|---|
| direct `[disp16]` | y | y | y |
| bp-relative `[bp+d8]`, `[bp+d16]` | y | y | y |
| pair `ax:dx` | y | y | y |
| pair `cx:bx` | y | y | y |
| pair-to-pair copy | y | y | y |
| cross-pair operation | y | y | y |

**Argument push** -- the one shape an option decides.

| | `/G3` | `/G2` | no switch |
|---|---|---|---|
| VBDOS | 32-bit push | two-word push | two-word push |
| PDS 7.1 | rejected | two-word push | two-word push |
| QB 4.5 | rejected | rejected | two-word push |

### What real code actually contains

A mixer-shaped loop -- long arithmetic, branches, a nested loop -- compiled
and put to `qbeStudy`: **45 blocks, 2 regions, 1 taken, 22 bytes of 900.**
One statement out of nine was lifted. That number reorders everything here,
so it is worth reading the shapes it found rather than the summary.

The one taken is `ph = ph + stp`, both operands in memory:

    005A  A1 5A 00        mov ax,[stp]
    005D  8B 16 5C 00     mov dx,[stp+2]
    0061  03 06 56 00     add ax,[ph]
    0065  13 16 58 00     adc dx,[ph+2]
    0069  A3 56 00        mov [ph],ax
    006C  89 16 58 00     mov [ph+2],dx

The rest are four shapes the lift does not know, in order of how often they
turned up:

| shape | what BC emits | state |
|---|---|---|
| immediate ALU pair | `2D <imm16>` / `83 DA <imm8>` -- `sub ax,imm` / `sbb dx,imm` | y |
| pair pushed to a call | `52` `50` -- `push dx` / `push ax` | no |
| immediate pushed to a call | `66 68 <imm32>` | no |
| comparison | `push`/`push`/`call B$CPI4` | no |

The immediate ALU pair is done. It bought chaining rather than the operation
itself: three statements of constant arithmetic with no branch between them
went from 3 regions and **nothing taken** to one region and 75 bytes to 53.
The constants had been ending every region, leaving each piece too small to
be worth rewriting.

It does not fire where a constant follows a branch, which is most of the
mixer loop. The pair holds no known value at a block's head, and widening
there would mean assembling `eax` out of `ax` and `dx` first -- more than it
saves. So the loop census above is unchanged by it.

The other three still reduce to one missing idea: an operand can be a
constant or a register pair, not just a memory reference. `qbo$mul` matches
`66 FF 36 <disp16>` twice and nothing else, so a multiply by a variable is
absorbed and a multiply by 3 is not.

### Running the pass twice buys nothing

Measured, not assumed. Two `qbeRun` calls over the same window:

    pass1   regions 2  taken 2  bytes 133 -> 107   blocks 3
    pass2   regions 0  taken 0  bytes   0 ->   0   blocks 5

Safe -- the values still agree -- and useless. Worse than useless: the block
count goes up, because the jump over a region's slack is itself a block
terminator, so pass 1 leaves the code more fragmented than it found it.

The reason is structural, and it bounds what any amount of iteration can do.
The pass rewrites in place and cannot move code, so the bytes a region saves
become slack inside that region and cannot be lent to its neighbour. Nothing
a later pass sees is any freer than what the first one saw. Chaining more
into a single region is the only way to spend those bytes, which is what
handling more operand shapes does.

This is also why liveness across blocks earned so little. It was built
against a measurement -- 14 bytes of a 43-byte chain going back out as
fixups -- that turned out to be a case where `dx` was genuinely live: BC
keeps the pair across the `IF` and reads it on the other side. The analysis
is sound and the unit test shows it drops a fixup when the successors really
do overwrite the pair. There is just far less of that than of the four rows
above.

### Procedure prologues -- a hook the compilers do commit to

Measured on all three, one module with a `SUB` and a `FUNCTION`. Every
procedure body begins with a frame size in `cx` and a far call into the
runtime, and ends with a far call and a `retf n`. The bodies are laid out one
after another with a `jmp` around each, so a linear scan meets them all.

| | prologue | bytes |
|---|---|---|
| VBDOS | `B9 <size>` `BB <imm>` `9A <far>` | 11 |
| PDS 7.1 | `B9 <size>` `9A <far>` | 8 |
| QB 4.5 | `B9 <size>` `9A <far>` | 8 |

    0094  B9 08 00        mov  cx,8            ; VBDOS, FUNCTION Beta
    0097  BB 00 00        mov  bx,0
    009A  9A 97 02 0D 00  call far 000D:0297   ; into the runtime
    ...
    00BF  9A E8 02 0D 00  call far 000D:02E8   ; and the exit
    00C4  CA 02 00        retf 2

The wrinkle: the target is not one address. All three runtimes export
`B$FRAMESETUP`, and one procedure kind enters it a fixed distance in --
`+1Fh` on VBDOS and PDS, `+1Ah` on QB 4.5 -- while the other kind calls a
different routine altogether. So a hook cannot match one address the way
`qbo$mul` matches `B$MUI4`. It can match the shape, or match any far call
into the runtime's code segment from a site that looks like a prologue,
which is the stronger signature of the two.

What it would buy, if taken: the window `qbeRun` is given is currently the
caller's to get right, and getting it wrong rewrites the BASIC runtime -- the
hazard that hung PDS 7.1 and quietly survived on the other two. A prologue
gives a procedure's true extent, so the pass could run per procedure on first
entry and never wander past the end of the module. That is a bigger change
than anything else on this list and is not started.

### Analysis

| | state |
|---|---|
| instruction length decoder | y, checked against ndisasm on 5888 instructions |
| basic blocks | y, calls fall through, indirect jumps end one |
| lift to 32-bit values | y |
| value liveness within a region | y |
| register model for unlifted code | y |
| flag liveness | y |
| dropping a dead `dx`/`bx` fixup | y |
| liveness across regions | no -- measured, buys nothing where it was tried |
| liveness across blocks | y -- and it buys 14 bytes; see the census below |
| regions something branches into | y, refused |
| register allocation beyond BC's choice | no -- what `MOD` on VBDOS needs |

### Testing

| | state |
|---|---|
| unit tests against the assembly | y, `src/test/qbeunit.bas` |
| whole-program regression | y, `src/test/qbeopt.bas` |
| run on all three compilers | y, `tools/qbe/regress.sh` |
| run on every switch that changes the code | y, `tools/qbe/matrix.sh`, 12 configurations |
| decoder against ndisasm | y, `tools/qbe/dectest.py` |
| cycle pricing | y, `tools/qbe/price.py` |
| parity benchmark | y, `tools/qbe/parity.bas` |
| semantic differential on real hardware | no -- the prefix question needs it |

## Where this stands

    long / integer      before    after    goal
    bitwise, additive     1.96     1.37     1.0
    multiply, divide      5.62     3.27     1.0

`tools/qbe/parity.sh`, VBDOS `/G3`. That is the number the whole thing is
for: longs are avoided in mgl *because* they cost double, so the point is
not to speed up code that already avoids them -- it is to stop the
avoidance being necessary.

Treat any parity figure older than `fb9331c` as unreliable. TIMER ticks at
18.2Hz and the sections used to run for 380ms, so one tick was 14% of a
reading and the ratio carried 0.2 of error. The 1.125 and 2.94 recorded
when the benchmark landed were a favourable rounding, and today's numbers
briefly looked like a regression against them until the emitted bytes
turned out to be identical.

## Done

Each of these was measured, not assumed, and each has a test that fails
without it.

| | evidence |
|---|---|
| absorb `*`, `\`, `MOD` | both push shapes; instruction parity with the integer form |
| lift to 32-bit values | a DAG over the two pairs, not a peephole |
| value liveness, flags, register model | region-accurate |
| `cx:bx` as a second pair | correct, and fires on nothing -- BC never stores it |
| bp-relative operands | spills read as values |
| put `dx`/`bx` back when live | and refuse the region when it will not fit |
| liveness across blocks | 14 bytes on one of twelve configurations |
| immediate operands | 3 regions and nothing taken became 1 region, 75 bytes to 53 |
| refuse a region something branches into | no configuration has hit it; insurance |

Two of those deserve their qualifiers. Liveness across blocks earns almost
nothing because BC keeps a long in its pair across control flow far more
often than expected, so `dx` really is live. Immediate operands buy
chaining rather than the operation: constants had been ending regions and
leaving each piece too small to rewrite.

## Next, in order

**1. Comparison.** The largest single item left. Seven of the thirty-four
surviving calls in the operator census are `B$CPI4`, it is the commonest
thing anyone does with a long, and the replacement is *smaller* than what
it replaces:

    push dword [b] / push dword [a] / call B$CPI4     15 bytes
    mov eax,[a] / cmp eax,[b]                          9

`B$CPI4` is another eleven instructions behind that call, rebuilding flags
through `lahf`/`sahf` because an 8086 cannot compare a long in one go. A 386
can, and the flags a `cmp` leaves are the ones the `jcc` after it wants.

**2. Operands that are not memory.** One missing idea behind three shapes,
all counted in the loop census below: a constant pushed to a call
(`66 68 <imm32>`), a pair pushed to a call (`52` `50`), and thereby multiply
and divide by a constant, and strength reduction after that. `qbo$mul`
matches `66 FF 36 <disp16>` twice and nothing else.

**3. The `not` pair.** Three bytes against four on its own, but `NOT`, `EQV`
and `IMP` all go through it -- and an unlifted idiom invalidates the
register tracking and costs everything after it until the next load. Adding
the long negate took a test from five regions to one.

**4. Register allocation.** Everything targets `eax`; values stay in the
pair BC chose. This is what `MOD` on VBDOS needs, and what would let a
consumer be handed a value where it wants it instead of through `dx`.

**5. The `66` prefix on real hardware.** Every cycle figure here is a model.
On 486 and P5 the prefix may cancel the widening win outright, and nothing
short of the metal will settle it.

## What the loop census reordered

A mixer-shaped loop -- long arithmetic, branches, a nested loop -- put to
`qbeStudy`: **45 blocks, 2 regions, 1 taken, 22 bytes of 900.** One
statement of nine. Immediate operands did not move it, because its
constants follow branches: at a block's head the pair holds no known value,
and widening there would mean assembling `eax` out of `ax` and `dx` first,
which costs more than it saves.

So the remaining wins are in items 1 and 2, not in more analysis. Real
BASIC scatters its long arithmetic where the benchmark chains it.

## What running the pass twice would buy

Nothing, measured:

    pass1   regions 2  taken 2  bytes 133 -> 107   blocks 3
    pass2   regions 0  taken 0  bytes   0 ->   0   blocks 5

Safe, and worse than useless: the block count rises, because the jump over
a region's slack is itself a block terminator. The pass rewrites in place
and cannot move code, so bytes a region saves become slack inside it and
cannot be lent to a neighbour. Chaining more into one region is the only
way to spend them -- which is why item 2 is worth more than any further
analysis.

## Still open

- **Measure against a real program.** Every run count here comes from
  generated tests. MODPLAY is the obvious target: 54 regions and 438 bytes
  of long arithmetic in 15795 of code, 37 of them one value and 17 of two.
- **The instruction count differs by two** past the first decoder bail;
  below that the implementations agree exactly, so it is bookkeeping in the
  retry rather than decoding.

## The standing question

All of this is easier on the `.OBJ` files between BC and LINK: call sites
come exactly from FIXUPP records instead of being matched, relocations are
visible rather than a hazard, no runtime footprint, no self-modifying code,
and tests run on the host in milliseconds instead of through DOSBox. The
decoder, matcher, liveness rules and flag facts port over unchanged; only
the assembly does not.

Staying at runtime needs nothing of whoever builds the program, which is
the reason it exists in this form. Worth re-deciding before step 5, where
the runtime approach starts costing real footprint.

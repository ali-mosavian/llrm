# What optimal looks like, per suite program

Written by hand against BC's own output, because the measurements in this
project were wrong for months in the same direction: every one asked a
question shaped so BC's style could not answer it, and the roadmap concluded
BC leaves nothing on the table. BC is not an optimising compiler. When a
measurement says otherwise, the measurement is what to suspect.

So these are targets rather than measurements. Each is BC's own loop body,
and beside it what the same loop should be. Counts are instructions in the
body and bytes; the bytes matter less than the reloads, since a reload in a
loop costs a memory access every iteration.

## hotlop -- a loop-invariant product

`s = s + (n * k) + i`, twenty passes. `n` and `k` are constants and neither
is written in the loop.

```
0048  mov ax,[n]          | mov  bx,[s]        ; hoisted
004b  imul word [k]       | mov  ax,1
004f  add ax,[s]          | loop:
0053  add ax,[i]          |   add  bx,21       ; n*k, folded and hoisted
0057  mov [s],ax          |   add  bx,ax
005a  mov ax,[i]          |   inc  ax
005d  inc ax              |   cmp  ax,20
005e  mov [i],ax          |   jle  loop
0061  cmp ax,14h          | mov  [s],bx
0064  jle short 0048      |
```

**10 instructions and 30 bytes, against 5 and about 12.** The `imul` is the
expensive part: it runs twenty times over two constants. Wanted: LICM,
constant folding, and keeping `s` and `i` in registers across the loop.

## press -- eight live variables, every product invariant

`r = r + a*b + c*d + e*f + g*h`, ten passes, nothing in the body written by
the loop.

```
006c  mov ax,[a]          | mov  bx,750       ; the whole sum, folded
006f  imul word [b]       | mov  ax,1
0073  mov bx,ax           | loop:
0075  mov ax,[c]          |   add  [r],bx
0078  imul word [d]       |   inc  ax
007c  add bx,ax           |   cmp  ax,10
007e  mov ax,[e]          |   jle  loop
0081  imul word [f]       |
0085  add bx,ax           |
0087  mov ax,[g]          |
008a  imul word [h]       |
008e  add bx,ax           |
0090  add [r],bx          |
0094  mov ax,[i]          |
0097  inc ax              |
0098  mov [i],ax          |
009b  cmp ax,0Ah          |
009e  jle short 006c      |
```

**18 instructions and 51 bytes, against 4 and about 9**, and four `imul`s
per pass become none. Sixteen operand loads per iteration, of eight
variables that never change. This is the program that says BC does no
register allocation across a statement: `bx` is the only value it keeps, and
only because the four products are one expression.

## arridx -- an array element addressed three times

`a(i) = i * 3` then `t = t + a(i) + a(i)`.

```
003c  mov cx,3            | mov  ax,1         ; i
003f  imul cx             | mov  si,2         ; &a(i)
0041  mov si,[i]          | mov  dx,3         ; 3i
0045  shl si,1            | loop:
0047  mov [si],ax         |   mov  [si],dx
004b  mov ax,[si]         |   mov  cx,dx
004f  add ax,ax           |   add  cx,cx
0051  add [0],ax          |   add  [t],cx
0055  mov ax,[i]          |   add  dx,3
0058  inc ax              |   add  si,2
0059  mov [i],ax          |   inc  ax
005c  cmp ax,14h          |   cmp  ax,20
005f  jle short 003c      |   jle  loop
```

**11 instructions and 36 bytes, against 9 and about 20.** Three separate
reloads of `i`, one of them one instruction after `i` was last in `ax`; a
reload of `a(i)` on the line after it was stored; and an `imul` where an
induction variable would do. Wanted: store-to-load forwarding, copy
propagation, and strength reduction of the subscript.

## subexp -- constants across statements

`x = 11 : y = 5 : p = (x+y)*2 : q = (x+y)*3`. BC does keep `x+y` in `bx`
across the two statements, which is more than this document assumed before
reading its output -- the miss here is that all four values are constants.

```
0030  mov word [x],0Bh    | mov  word [x],11
0036  mov word [y],5      | mov  word [y],5
003c  mov ax,[x]          | mov  word [p],32
003f  add ax,[y]          | mov  word [q],48
0043  mov bx,ax           |
0045  shl ax,1            |
0047  mov [p],ax          |
004a  mov ax,3            |
004d  imul bx             |
004f  mov [q],ax          |
```

**10 instructions and 34 bytes, against 4 and 24.** Wanted: constant
propagation with a consumer that emits from it.

## ivchan -- a chain of derived induction variables

`p = i*12`, `q = p+5`, `a(q) = i`, `t = t + a(q)`. Four affine functions of
one counter: i, 12i, 12i+5, and the element's own address at 24i+10.

```
003c  mov cx,0Ch          | xor  ax,ax        ; i
003f  imul cx             | mov  bx,10        ; &a(q) = 24i+10
0041  mov [p],ax          | loop:
0044  add ax,5            |   mov  [bx],ax    ; a(q) = i
0047  mov [q],ax          |   add  [t],ax     ; already in ax
004a  mov si,ax           |   add  bx,24
004c  shl si,1            |   inc  ax
004e  mov ax,[i]          |   cmp  ax,20
0051  mov [si],ax         |   jle  loop
0055  mov cx,[si]         |
0059  add [t],cx          |
005d  inc ax              |
005e  mov [i],ax          |
0061  cmp ax,14h          |
0064  jle short 003c      |
```

**14 instructions and 41 bytes, against 6 and about 14.** The `imul` becomes
an `add`; the reload of `i` at 004e is one instruction after `i` was last in
`ax`; the reload of `a(q)` at 0055 is one after storing it; and the stores to
`p` and `q` are dead, since nothing reads either after the loop.

Wanted together: induction-variable recognition to see all four are affine,
strength reduction to increment rather than multiply, store-to-load
forwarding, and dead-store elimination.

## stride -- a division that is really a counter

`FOR i = 0 TO 100 STEP 5`, and `b(i) = i \ 5`.

```
003c  mov cx,5            | xor  ax,ax        ; i
003f  cwd                 | xor  dx,dx        ; k = i\5
0040  idiv cx             | xor  si,si        ; &b(i)
0042  mov si,[i]          | loop:
0046  shl si,1            |   mov  [si],dx
0048  mov [si],ax         |   add  [t],dx
004c  mov ax,[si]         |   add  si,10
0050  add [t],ax          |   inc  dx
0054  add cx,[i]          |   add  ax,5
0058  mov ax,cx           |   cmp  ax,100
005a  mov [i],ax          |   jle  loop
005d  cmp ax,64h          |
0060  jle short 003c      |
```

**12 instructions and 37 bytes, against 7 and about 17** -- and an `idiv`,
around forty cycles on a 386, becomes an `inc`. This is scalar evolution in
its plainest form: `i \ 5` where `i` strides by five is the sequence
0, 1, 2, ..., and nothing about it needs a division.

## matrix -- two dimensions by hand

`m(r * w + c) = r + c` in a nested loop, then a diagonal read `m(r*w + r)`.

The inner loop, ten instructions and 37 bytes:

```
0048  add ax,[c]          | ; si = r*w*2 and ax = r+c, both set outside
004c  mov bx,ax           | loop:
004e  mov ax,[r]          |   mov  [si],ax
0051  imul word [w]       |   inc  ax
0055  mov si,ax           |   add  si,2
0057  add si,[c]          |   dec  cx
005b  shl si,1            |   jnz  loop
005d  mov [si],bx         |
0061  mov ax,[c]          |
0064  inc ax              |
0065  mov [c],ax          |
0068  cmp ax,13h          |
006b  jle short 0048      |
```

**10 against 5**, and `imul word [w]` -- invariant in the inner loop -- runs
four hundred times. The diagonal loop is the same shape with a stride of
`2 * (w + 1)`: **10 instructions against 6**, one more `imul` gone.

## Registers, which is the whole of it

BC uses `ax` for everything, `bx` when it needs a second operand alive, and
`cx` or `dx` only where the instruction forces it -- `imul`'s implicit
operand, `idiv`'s dividend. `si` is an address scratch. It emits a statement
at a time, so a value lives in a register for as long as one statement needs
it and is written back before the next.

Measured over the seven programs here: **eight loops, seven of which touch
no more variables than there are registers, and BC uses between two and
four.** 52 memory accesses in them would become none.

| program | variables the loop touches | registers BC uses | an allocation that fits |
|---|---|---|---|
| hotlop | 3 (`s`, `i`, and the folded `n*k`) | 4 | `bx`=s, `ax`=i, `dx`=21 |
| arridx | 4 (`i`, `t`, `a(i)`, the address) | 4 | `ax`=i, `si`=&a(i), `dx`=3i, `bx`=t |
| press | 10 | 3 | `bx`=the folded sum, `ax`=i, `dx`=r |
| ivchan | 4 (`i`, `t`, `p`, `q`) | 4 | `ax`=i, `bx`=&a(q), `dx`=t |
| stride | 4 (`i`, `t`, `b(i)`, the address) | 4 | `ax`=i, `dx`=i\5, `si`=&b(i), `bx`=t |
| matrix inner | 4 (`r`, `c`, `w`, the address) | 4 | `ax`=r+c, `si`=address, `cx`=count, `dx`=r |
| matrix outer | 3 | 2 | `dx`=r, `si`=row base |

`press` is the one to keep: ten variables against six registers, so it is
the only program here that genuinely needs a spill -- and once the invariant
sum is folded, one register holds it and the loop touches three things.
Every other loop fits entirely, and none of them is allocated.

## spill -- which variable to spill, and what it costs

Eight variables and six registers, and they are not equally worth keeping.
`h1`, `h2` and `h3` are read once per inner iteration, a hundred times;
`o1` and `o2` ten times in the outer loop. `tools/opportunity.py --spill`
ranks them by accesses weighted ten per level of nesting, which is the
number an allocator spills by:

```
   202  t          inner
   200  j          inner
   101  h1         inner
   101  h2         inner
   101  h3         inner
    31  o1         outer
    22  o2         outer
    20  i          outer
```

A ten to one separation, and BC uses none of it. Its inner loop:

```
006c  mov ax,[h1]         | ; before the loop: dx = h1*h2+h3, folded to 22
006f  imul word [h2]      | ; bx = t, cx = j
0073  add ax,[h3]         | loop:
0077  add ax,[t]          |   add  bx,dx
007b  mov [t],ax          |   inc  cx
007e  mov ax,[j]          |   cmp  cx,10
0081  inc ax              |   jle  loop
0082  mov [j],ax          |
0085  cmp ax,0Ah          |
0088  jle short 006c      |
```

**9 instructions and 30 bytes, against 4 and about 9**, and an `imul` per
pass over three constants.

The allocation, at each point: `bx` holds `t` and `cx` holds `j` for the
whole nest; `dx` holds the folded invariant; `ax` is the outer counter `i`;
`si` holds `o1`. That leaves `o2` in memory -- the right choice, because it
costs 22 and the cheapest of the others costs 101. **Spilling by cost spills
`o2`; spilling by BC's rule spills all eight, 778 weighted accesses instead
of 22.**

## split -- two variables, one register

`a` is dead before `b` is born: `a` is read only in the first loop and `b`
only in the second, so one register holds both and nothing is spilled. Live
ranges that do not overlap can share, which is the half of allocation that
is not about pressure at all -- and BC, having no live ranges, has neither
half.

## segld -- the segment reload BC hides

A dynamic array reaches its elements through a descriptor, and BC reloads
`es` from it on every subscript. Twice in one statement where the element
appears twice, and the descriptor is written once by `B$DDIM` before either
loop runs.

```
0054  shl ax,1            | ; before both loops:
0056  mov bx,ax           |   mov  si,0
0058  mov si,0            |   mov  es,[si+2]      ; once
005b  add bx,[si+0Ah]     |   mov  di,[si+0Ah]    ; once
005e  mov es,[si+2]       | inner:
0061  mov cx,[i]          |   mov  [es:di],cx
0065  mov [es:bx],cx      |   add  dx,cx
0068  mov bx,ax           |   add  di,2
006a  add bx,[si+0Ah]     |   inc  cx
006d  mov es,[si+2]       |   cmp  cx,20
0070  mov ax,[es:bx]      |   jle  inner
0073  add [t],ax          |
0077  inc cx              |
0078  mov ax,cx           |
007a  mov [i],ax          |
007d  cmp ax,14h          |
0080  jle short 0054      |
```

**17 instructions and 7,100 weighted cycles in the inner loop, against 6 and
1,600.** Two `mov es` per pass over a five-by-twenty nest is **two hundred
segment loads of a word nothing writes**, and `mov si,0` re-materialises the
descriptor's own address every time.

None of the cell counters see any of it: the descriptor is reached through a
base register, so it has no address they can name. That is what
`segment register reloaded inside a loop` is for, and it is the measure this
file was missing -- BC hides the cost inside `B$HARR` and the descriptor,
and PEEK and POKE reload their segment on every use for the same reason.

## harr -- the offset recomputed for every use

Two dimensions and dynamic. The element's address is affine in the inner
counter with a stride of two, and affine in the outer with a stride of twice
the row width -- so one `add` per pass is what it costs once an induction
variable carries it. BC recomputes the whole thing from the subscripts:

```
0058  add ax,[c]          | ; outside both loops: es, and di = array base
005c  mov bx,ax           | ; outer: bx = &m(r,1), ax = r+1, cx = 10
005e  mov ax,[r]          | inner:
0061  imul word [w]       |   mov  [es:bx],ax
0065  add ax,[c]          |   add  dx,ax
0069  shl ax,1            |   inc  ax
006b  mov dx,bx           |   add  bx,2
006d  mov bx,ax           |   dec  cx
006f  mov si,0            |   jnz  inner
0072  add bx,[si+0Ah]     |
0075  mov es,[si+2]       |
0078  mov [es:bx],dx      |
007b  mov bx,ax           |
007d  add bx,[si+0Ah]     |
0080  mov es,[si+2]       |
0083  mov ax,[es:bx]      |
0086  add [t],ax          |
008a  mov ax,[c]          |
008d  inc ax              |
008e  mov [c],ax          |
0091  cmp ax,0Ah          |
0094  jle short 0058      |
```

**22 instructions and 11,700 weighted cycles in the inner loop, against 6
and 1,600.** Per pass, over a hundred of them:

- one `imul word [w]` to recompute the row offset
- `mov si,0` to re-materialise the descriptor's own address
- two `add bx,[si+0Ah]` for the array base, one per subscript in the statement
- two `mov es,[si+2]`, likewise

That is a multiply, two segment loads and two descriptor reads where an
induction variable with its strength reduced uses **one add**. PDS inlines
this; on a compiler that calls `B$HARR` the same arithmetic happens inside
the helper, which is where it is easiest to miss -- the cost does not appear
in the caller's own instructions at all.

`a multiply or divide inside a loop` counts the first of these directly: an
address affine in the counter has no business being recomputed.

## The scoreboard

Every target below is hand-derived from the full listing, both sides: BC's
own body costed instruction by instruction, and the optimal listing costed
the same way. Nothing here is scaled from another program's savings, which
four of them were -- and every one of those four was too generous, by
between 1.5 and 2.3 times. Estimating the target flattered BC.

```
  program       cost  target  ratio   redundancy left
  lngmix        1504     210   7.2x    0
  nested       12826    1850   6.9x   14
  spill         7458    1160   6.4x    8
  press         1788     315   5.7x   11
  matrix        8540    1750   4.9x   12
  split         1196     300   4.0x    9
  hotlop         792     215   3.7x    6
  stride        1060     360   2.9x    5
  addrm         1296     470   2.8x   12
  ivchan         930     340   2.7x    4
  rotate         826     345   2.4x    9
  arridx         850     400   2.1x    5
  bools          210     126   1.7x   13
  subexp         203     162   1.3x    2
  segld         7850    1925   4.1x   12
  harr         12454    1900   6.6x   18
```

**BC runs between 1.3 and 7.2 times the cost of code written by hand.** The
worst four are the two nested loops and the two about registers, which is
what a compiler that never keeps a value past a statement costs when the
statement is inside something that repeats.

`lngmix` at 7.2x is the one worth reading twice: its loop body is two long
runtime calls over an operand that never changes, so absorption, LICM and
folding compound -- 1,380 of its 1,504 is a loop body that should not run at
all. It is also the only program here with *no* redundancy left by the
counters, which says plainly that the counters and the cost are asking
different questions and both are needed.

Two of the hand totals came out exactly on the tool's number (`lngmix`,
1504) and the rest within three per cent, the difference being BC's header
bytes decoding as instructions before the first real one. Where they differ
the target is the hand ratio applied to the measured cost.

## The number to minimise## The number to minimise

`tools/opportunity.py` prints a weighted cycle count: an operation's own
cost, four more for each memory operand, and ten per level of loop nesting
as a stand-in for a trip count nothing here knows. `imul` is 22 and `idiv`
43, which is why a division in an inner loop outweighs a hundred
straight-line moves.

The absolute figure means little. What it is for is ranking the same program
compiled two ways, and it is the one number these targets roll up into.

## What these add up to

| | BC | optimal | wanted |
|---|---|---|---|
| hotlop loop | 10 insns, 30 B | 5, ~12 | LICM, folding, register promotion |
| press loop | 18 insns, 51 B | 4, ~9 | LICM, folding, register promotion |
| arridx loop | 11 insns, 36 B | 9, ~20 | store-to-load, copy propagation, strength reduction |
| subexp | 10 insns, 34 B | 4, 24 | constant propagation with a consumer |
| ivchan loop | 14 insns, 41 B | 6, ~14 | induction variables, IVSR, store-to-load, dead stores |
| stride loop | 12 insns, 37 B | 7, ~17 | scalar evolution: a divide that is a counter |
| matrix inner | 10 insns, 37 B | 5, ~11 | LICM out of the inner loop, IVSR on the address |
| matrix diagonal | 10 insns | 6 | IVSR with a stride of 2(w+1) |
| spill inner | 9 insns, 30 B | 4, ~9 | spill by cost: one variable, not eight |
| split | two loops | one register for both | live-range splitting |
| segld inner | 17 insns, 7100 | 6, 1600 | hoist es and the array base out of the nest |
| harr inner | 22 insns, 11700 | 6, 1600 | one add, not a multiply and two segment loads |

Roughly **half to three-quarters of the loop bodies**, and every `imul` and
`idiv` in all of them. The multiplies are not incidental: BC emits one per
subscript per statement, so a two-dimensional access in a nested loop is a
multiply four hundred times over an operand the loop never writes. Nothing in this project comes close to that today: what
it does is absorb runtime calls and widen long pairs, both of which are real
and neither of which touches any of the above.

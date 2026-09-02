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

## The number to minimise

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

Roughly **half to three-quarters of the loop bodies**, and every `imul` and
`idiv` in all of them. The multiplies are not incidental: BC emits one per
subscript per statement, so a two-dimensional access in a nested loop is a
multiply four hundred times over an operand the loop never writes. Nothing in this project comes close to that today: what
it does is absorb runtime calls and widen long pairs, both of which are real
and neither of which touches any of the above.

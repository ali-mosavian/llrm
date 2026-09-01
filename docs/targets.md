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

## What these add up to

| | BC | optimal | wanted |
|---|---|---|---|
| hotlop loop | 10 insns, 30 B | 5, ~12 | LICM, folding, register promotion |
| press loop | 18 insns, 51 B | 4, ~9 | LICM, folding, register promotion |
| arridx loop | 11 insns, 36 B | 9, ~20 | store-to-load, copy propagation, strength reduction |
| subexp | 10 insns, 34 B | 4, 24 | constant propagation with a consumer |

Roughly **half to three-quarters of the loop bodies**, and every `imul` in
three of the four. Nothing in this project comes close to that today: what
it does is absorb runtime calls and widen long pairs, both of which are real
and neither of which touches any of the above.

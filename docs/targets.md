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

## Event-enabled configurations need separate references

The listings below do not account for `/V` or `/W` event checks. Comparing
an event-enabled build to those plain-program denominators does not measure
the same semantics. The scoreboard now detects the actual module header's
event bits, not an `-evt` filename, and marks such comparisons PROVISIONAL.
It keeps their measured costs, suppresses the ratios, and cannot pass them
as complete even if their cost happens to be below a plain target.

Before: optimized QB BOOLS with event checks was reported as
`576 / 126 = 4.57x`. After: its cost remains **576**, but its ratio is
unverified until a complete event-preserving reference is derived. Plain
QB BOOLS remains **172 / 126 = 1.37x**. This changes no emitted assembly.
Emission refusals remain UNMEASURED, not provisional successes or optimized
fallbacks. Event semantics and unsupported event paths still need work.

The fail-first instrument regressions cover renamed objects from QB, PDS
and VBDOS, plus a below-target cost that must still fail completion. No
denominator was increased or inferred from current output.

## FPDEEP — exact constant floating expressions

For ordinary unchecked, event-free builds, the three array elements are
12, 28 and 60, `k=4`, and `d=12`; no numeric address escapes. Expand the
three known iterations in source order. Every intermediate product, sum,
difference and quotient below is exactly representable even with a 24-bit
significand. SINGLE stores and CLNG therefore do not change the answers,
regardless of rounding mode. There is no division by zero, overflow,
underflow or cancellation to signed zero. No reassociation is needed.

| i | p | p*p | (p*p)/(p+p) | (p-4)/(p+4) | MIX after multiplying by 1024 |
| --- | --- | --- | --- | --- | --- |
| 1 | 12 | 144 | 6 | 1/2 | 512 |
| 2 | 28 | 784 | 14 | 3/4 | 768 |
| 3 | 60 | 3600 | 30 | 7/8 | 896 |

Emit SQ, RATIO, MIX for i=1, then i=2, then i=3. Follow with DSQ=144,
DRATIO=6, DONE and termination. Keep each runtime printing call, including
the index and equals descriptor, so spacing and numeric formatting are
unchanged. Calls below were checked against all three ordinary BC objects.

```asm
; Each of the nine indexed rows: 104 ranking units
push word labelDescriptor      ; 6
call far B$PSSD                ; 20
push word index                ; 6
call far B$PSI2                ; 20
push word equalsDescriptor     ; 6
call far B$PSSD                ; 20
push dword result              ; 6
call far B$PEI4                ; 20

; DSQ and DRATIO: 52 each
push word labelDescriptor      ; 6
call far B$PSSD                ; 20
push dword result              ; 6
call far B$PEI4                ; 20

; Epilogue: 46
push word doneDescriptor       ; 6
call far B$PESD                ; 20
call far B$CEND                ; 20
```

Complete target: **9×104 + 2×52 + 46 = 1086**. This is the same static
ranking used by the scoreboard, not elapsed time or a hardware-cycle claim.
The optimized program has not reached this form: opaque scalar copies and
missing array/value facts still block its floating constant propagation.
Event-enabled builds remain provisional because their event observations
cannot be discarded by this reference.

## NOTS and NEGNOT — constant expressions across output statements

These ordinary, unchecked, event-free programs initialize two unescaped
LONG scalars, then print expressions over them. Their sources contain no
READ, user callback, assignment to either input after initialization, or
runtime-dependent arithmetic. A source-level compiler can evaluate every
expression modulo 32 bits. This reference does not grant qbopt permission
to assume arbitrary machine-level runtime calls preserve arbitrary memory;
recovering that source-level fact is part of the remaining work.

The source never exposes the numeric variables' addresses, so their stores
are dead once each expression is folded. Print the same descriptors and signed
LONG bit patterns through the same runtime entry points. The complete reference
is the per-row sequence for every row in order and the common epilogue.
Each instruction's cost uses this tool's ranking formula, not hardware timing.

```asm
; Each row: 52; no initialization or result stores needed
push word descriptor           ; 6
call far B$PSSD                 ; 20
push dword value               ; 6, same bytes as high-word then low-word push
call far B$PEI4                 ; 20

; Both programs: epilogue = 46
push word descriptorDone       ; 6
call far B$PESD                 ; 20
call far B$CENP                 ; 20
```

| Program | Descriptor | Source expression | LONG bits |
| --- | --- | --- | --- |
| NOTS | NOT= | NOT a | EDCBA987 |
| NOTS | EQV= | a EQV b | E2C4A688 |
| NOTS | IMP= | a IMP b | EFCFAF8F |
| NOTS | NAND= | NOT (a AND b) | FDFBF9F7 |
| NOTS | NOTOR= | (NOT a) OR b | EFCFAF8F |
| NEGNOT | A= | -(NOT a) | 12345679 |
| NEGNOT | B= | -(NOT (a AND b)) | 02040609 |
| NEGNOT | C= | NOT (a OR b) | E0C0A080 |
| NEGNOT | D= | -(a AND b) | FDFBF9F8 |

NOTS target: **5×52 + 46 = 306**.
NEGNOT target: **4×52 + 46 = 254**.
No result is carried in a register across a printing call.
The earlier targets 378 and 290 retained unnecessary stores and split pushes;
they were too generous. These corrected references use the backend's 386+
instruction set, without assuming a particular instruction latency.

The raw PDS listings cross-check as follows: NOTS has 12 calls, 56 other
instructions and 60 non-call memory reads/writes, hence
12×20 + 56×2 + 60×4 = **592**. NEGNOT has 10 calls, 46 other instructions
and 31 non-call memory reads/writes, hence 10×20 + 46×2 + 31×4 = **416**.
There are no loops or multiply/divide instructions to add another weight.
Targets are not scaled from the optimized output. Consult the current report
for ratios; the raw costs above describe BC, not the optimizer.

## ARITH — full constant-output reference

The ordinary unchecked, event-free source has no input or escaping numeric
variable. All arithmetic fits its intended LONG semantics, and all stores can
be eliminated after evaluation. Emit the 52-unit LONG row above for each of
these nine rows, in order:

| Descriptor | Source expression | LONG bits |
|---|---|---|
| AND= | a AND b | 02040608 |
| OR= | a OR b | 1F3F5F7F |
| XOR= | a XOR b | 1D3B5977 |
| ADD= | a + b | 21436587 |
| SUB= | a - b | 03254769 |
| NEG= | -a | EDCBA988 |
| CHAIN= | ((a AND b) XOR a) + b | 1F3F5F7F |
| CARRY= | 65535 + 1 | 00010000 |
| BORROW= | 65536 - 1 | 0000FFFF |

Then emit the two INTEGERs, preserving their distinct formatting entry points:

```asm
push word descriptorInts       ; 6
call far B$PSSD                 ; 20
push word 258                  ; 6
call far B$PSI2                 ; 20
push word 772                  ; 6
call far B$PEI2                 ; 20
```

Finish with the same 46-unit DONE/termination epilogue above. The complete
reference costs **9×52 + 78 + 46 = 592**: 23 calls at 20 and 22 pushes at 6.
Descriptor data and termination behavior remain unchanged. This is a static
reference, not a claim that all machine-level memory proofs are implemented.

### Scoring decoded instructions

The scorer now counts each reached decoded instruction, not raised MIR
operations. A synthetic `extract` added to NOTS's raised body previously
increased its score by two without changing any object byte. The fail-first
regression now leaves its score unchanged. Memory traffic is counted from
instruction accesses regardless of whether an address can be resolved.
This changes no assembly. FPCSE's current score changes from 491 to 539;
its old reference remains provisional. Existing targets are not increased
to compensate for changed measurements.

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

### Runtime-input twins need their own complete references

HOTLPX reads `n` and `k` through the runtime. Its old inherited 312 denominator
is replaced by the complete **217** reference below. The inputs remain unknown;
the DATA values are not folded into the answer.

With word wrapping, twenty iterations give
`s = (20 * ((n*k) mod 65536) + 210) mod 65536` and `i = 21`.
No intermediate arithmetic result is observed. This applies to the ordinary
unchecked, event-free builds, not event checks or resumable overflow handling.
The reference retains the final counter and sum stores before output.

```asm
; Input: 64. RDI2 consumes a stacked far pointer, no register inputs.
push ds                       ; 6
push word n                   ; 6, relocated offset
call far B$RDI2               ; 20
push ds                       ; 6
push word k                   ; 6, relocated offset
call far B$RDI2               ; 20

; Arithmetic and final state: 51. No loop remains.
mov ax,[n]                    ; 6
imul ax,[k]                   ; 26, low word product
lea ax,[eax+eax*4]             ; 2, low word is product * 5
shl ax,2                      ; 3, product * 20
add ax,210                    ; 2, sum of 1..20
mov [s],ax                    ; 6
mov word [i],21               ; 6

; Output and termination: 102. No value survives a runtime call in a register.
push word labelS              ; 6, original string descriptor offset
call far B$PSSD               ; 20
push word [s]                 ; 10, source load plus stack store
call far B$PEI2               ; 20
push word labelDone           ; 6, original string descriptor offset
call far B$PESD               ; 20
call far B$CENP               ; 20
```

These are the existing ranking costs, not hardware timing: ordinary operations
cost 2, shifts 3, IMUL 22, each memory access 4, and these runtime calls 20.
Both stack writes and the load in `push [s]` count. Input setup replaces
BC's `push ds / pop es / push es` by `push ds`: the stacked pointer is
identical, RDI2's established contract has no register inputs, and ES is
clobbered by the call in either case. The original module header, DATA and
string descriptors are retained; only executable instructions are priced.
The upper half of EAX does not affect LEA's low-word result.

Independent original-PDS accounting: entry/input/setup **104**, loop body
**58 * 20**, loop test **10 * 20**, output **102**, total **1566**. The model
weights loop blocks by twenty, including the test, rather than claiming an
exact dynamic trace. QB and VBDOS original totals are **1570/1576**.
Current emitted costs are **251/255/261**, so the new ratios are
**1.16x/1.18x/1.20x**; all three are within 1.5x. The goal as a whole is not
complete. This target change alters no generated code.

### LNGMXX -- runtime dividend, constant divisor

LNGMXX's independent reference is **208**, replacing the inherited 210.
For `q = trunc(v/7)` and `r = v - 7*q`, ten wrapped additions give
`s = 10*(q+r) = 10*(v-6*q)` modulo 2^32. The divisor is nonzero and cannot
trigger the signed MIN/-1 case. Final `i=11` and `s` remain stored.

LLVM's `llvm/lib/Support/DivisionByConstantInfo.cpp`, inspected at local commit
`338e0c9`, supplies the signed reciprocal algorithm. Apple Clang 21.0.0 with
`-target i386-unknown-linux-gnu -O2 -fwrapv -ffreestanding` on
`int f(int v) { return 10*(v/7+v%7); }` independently emitted the negative
magic multiply, sign correction and `v-6*q` form. The listing below places
the commutative multiply's magic constant directly in EAX, avoiding Clang's
extra operand copy, and includes BASIC input/output rather than a C return.

```asm
; Input: 32, same far-pointer convention as RDI2.
push ds                       ; 6
push word v                   ; 6
call far B$RDI4               ; 20

; Arithmetic and final stores: 64.
mov ecx,[v]                   ; 6
mov eax,092492493h            ; 2, signed -1840700269
imul ecx                      ; 22, signed high product in EDX
add edx,ecx                   ; 2
mov eax,edx                   ; 2
shr eax,31                    ; 3, sign correction
sar edx,2                     ; 3
add edx,eax                   ; 2, quotient truncated toward zero
add edx,edx                   ; 2
lea eax,[edx+edx*2]           ; 2, 6*q
sub ecx,eax                   ; 2, v-6*q = q+r
add ecx,ecx                   ; 2
lea eax,[ecx+ecx*4]           ; 2, ten times q+r
mov [s],eax                   ; 6
mov word [i],11               ; 6

; Output: 112, including both words of the LONG argument.
push word labelS              ; 6
call far B$PSSD               ; 20
push word [s+2]               ; 10
push word [s]                 ; 10
call far B$PEI4               ; 20
push word labelDone           ; 6
call far B$PESD               ; 20
call far B$CENP               ; 20
```

Original PDS accounting is **58 + 788*10 + 10*10 + 112 = 8150**;
QB/VBDOS total **8234/8070**. Current costs **248/250/246** yield
**1.19x/1.20x/1.18x** against 208. These are existing cost-model rankings,
not hardware timings. Our output still contains IDIV; the reference does not.
Boundary checks verify signed quotient correction and wrapped recurrence;
this target change does not implement reciprocal division in qbopt.

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

### PRESSX -- eight runtime inputs, four products

The complete reference is **508**, not PRESS's constant-folded 308.
Ten iterations are `r = 10*(a*b+c*d+e*f+g*h)` modulo 65536, with `i=11`.
All eight inputs remain unknown. Like HOTLPX, ordinary event-free word
arithmetic and the established stack-based READ contract are required.

```asm
; Input: 8 * 32 = 256. Each triple costs 6 + 6 + 20.
push ds
push word a
call far B$RDI2
push ds
push word b
call far B$RDI2
push ds
push word c
call far B$RDI2
push ds
push word d
call far B$RDI2
push ds
push word e
call far B$RDI2
push ds
push word f
call far B$RDI2
push ds
push word g
call far B$RDI2
push ds
push word h
call far B$RDI2

; Arithmetic and final stores: 150.
mov ax,[a]                    ; 6
imul ax,[b]                   ; 26
mov bx,[c]                    ; 6
imul bx,[d]                   ; 26
add ax,bx                     ; 2
mov bx,[e]                    ; 6
imul bx,[f]                   ; 26
add ax,bx                     ; 2
mov bx,[g]                    ; 6
imul bx,[h]                   ; 26
add ax,bx                     ; 2
add ax,ax                     ; 2
lea ax,[eax+eax*4]             ; 2, total factor ten
mov [r],ax                    ; 6
mov word [i],11               ; 6

; Output: 102, identical instruction costs to HOTLPX.
push word labelR              ; 6
call far B$PSSD               ; 20
push word [r]                 ; 10
call far B$PEI2               ; 20
push word labelDone           ; 6
call far B$PESD               ; 20
call far B$CENP               ; 20
```

Original PDS accounting: **380 + 154*10 + 10*10 + 102 = 2122**;
QB/VBDOS totals **2166/2132**. Current costs **627/631/637** give
**1.23x/1.24x/1.25x**. The higher denominator replaces an invalid reference
with a different computation's complete cost; it is not inferred from current
output. The earlier 2.31x comparison did not establish an optimization gap.

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
induction variable with its strength reduced uses **one add**.

**`B$HARR` does not appear here, and could not be made to.** Tried:
`REM $DYNAMIC` and `REDIM` with variable bounds, one and two dimensions, on
all four compilers, and the whole 215-object corpus -- zero call sites, and
the inner loop above is what every one of them emits. The allocator differs
(`B$DDIM` against `B$RDIM`); the access never becomes a call.

The untested lever is `/AH`, huge arrays over 64K, which no configuration
here passes and without which PDS refuses a 40,000-element array outright.
An element past a segment boundary needs arithmetic that cannot be inlined,
which is the shape a helper would exist for. If it is reached that way the
cost moves inside the helper and stops appearing in the caller's
instructions at all -- worth knowing, and not something this file has
measured.

`a multiply or divide inside a loop` counts the first of these directly: an
address affine in the counter has no business being recomputed.

## huge -- the helper that hides the whole cost

An array over 64K needs `/AH`, and then every element goes through
`B$HARY`, which takes the subscript as a long and hands back `es:bx`. The
long multiply and the segment normalisation happen inside it, so **none of
the cost appears in the caller's instructions at all** -- which is why every
measure in this file missed it until the helper was priced.

`bench/huge.bas`, on QuickBASIC 4.5 with `/AH`:

```
003c  mov cx,3E8h         | ; es and bx set once, outside
003f  imul cx             | loop:
0041  push ax             |   mov  [es:bx],ax
0044  mov ax,1            |   add  dx,ax
0047  push ax             |   add  bx,2000      ; the stride, with a
004a  mov bx,0            |                     ; segment bump when it wraps
004d  call B$HARY         |   inc  ax
0052  mov cx,[i]          |   cmp  ax,10
0056  mov [es:bx],cx      |   jle  loop
0059  push dx             |
005b  mov bx,0            |
005e  call B$HARY         |
0063  mov ax,[es:bx]      |
0066  add [t],ax          |
...
```

**Two `B$HARY` calls per iteration, for the same element, in the same
statement** -- one to store it and one to read it back. 3,944 weighted
cycles against about 320.

It is not in `suite/` because only QuickBASIC 4.5 accepts the form; PDS
refuses the REDIM with a math overflow, and the matrix compiles every suite
program on every configuration.

### What pricing the helpers changed

A call was charged a flat twenty, which is the `call` and not the callee.
`B$HARY` is a 32-bit multiply and a segment normalisation; `B$DVI4` and
`B$RMI4` are long division. With them priced, `lngmix` goes from 1,504 to
3,504 against the same target -- **16.7x, the worst in the whole set** --
because its loop body is two long divisions over an operand that never
changes.

That is the general lesson in this file, for the third time: a cost hidden
behind a call is a cost the measure has to be told about, or it reads as
free.

## What "optimal" means here, and why the ratio depends on it

Two answers, and this file has been giving the first without saying so.

**Keep the loop and compile it well** -- allocate the registers, hoist the
invariants, reduce the strength of the induction variables. That is what
every target above is, and it puts BC at three to seven times for integer
code and thirteen to seventeen where it calls the runtime.

**Compile it the way a modern optimiser does** -- and most of these loops do
not survive at all, because their results are compile-time constants.
`press` computes `r = 7500`. `matrix` computes `t = 380` and never reads the
array again, so the whole nest is dead. `fx`, a float loop, is
`r = 20` and its target drops from about 1,038 to about 144: **3.6x becomes
26x**.

Twenty to fifty times is the second answer, and it is the right one for real
programs. The first is what these targets measure.

**Every program in this suite is fully constant-foldable**, which makes the
second answer degenerate here -- deleting a loop is not a general result.
Measuring the achievable figure honestly needs inputs the compiler cannot
see: a value read from `DATA`, or a bound that is not a literal. That is the
next thing this suite needs, and until it has it the ratios below are a
floor.

## fpcse -- float is just as bad the moment a value crosses a statement

### Literal FPCSE and runtime-input FPCSEX need different references

The actual literal fixture initializes `a=2`, `b=4`, `c=8`, `s=0`, then
runs ten iterations. Its modern-compiler destination is constant evaluation,
not merely retaining `(a+b)` between statements. In source order:

- `a+b=6`, `p=48`, and `q=3/4`, including the SINGLE stores to p and q.
- After iteration k, `s=195*k/4`. The two individual additions are still
  evaluated as `(s+48)+3/4`, not reassociated.
- The largest intermediate numerator over denominator four is 1950. Every
  intermediate is exactly representable at 24 significant bits and within
  the normal SINGLE range. Each SINGLE store is exact too. No chosen rounding
  mode or extended precision is needed for these values.
- At exit, `s=975/2=487.5`, with SINGLE bits `0x43f3c000`, matching the golden
  output. The ten source-ordered steps were also checked using rational
  `floatfacts.evaluated`, including every storage conversion.

The intended reference therefore has no arithmetic loop: it prints this
constant through the existing runtime ABI. This is a numerical proof, not
yet an approved whole-program target listing. A complete reference must also
account for pending-exception synchronization, observable final stores and
the print/termination calls, and be linked and run before replacing the
provisional denominator. Do not replace 1340 with an estimated constant.

FPCSEX reads its inputs at runtime and cannot use this argument. It remains
the general floating value-numbering and loop-invariant-motion benchmark;
its strict storage rounding and exception obligations remain intact.

**LLVM cross-check, 2026-09-09:** local LLVM revision
`338e0c94943a6fb917c276bbbd9ff4b6cd6dd71e`,
`llvm/lib/Transforms/Scalar/EarlyCSE.cpp`, `SimpleValue::canHandle`, rejects
constrained floating add/subtract/multiply/divide with `ebStrict` exception
behavior or dynamic rounding. The current FPCSEX MIR explicitly carries both.
Thus ordinary LLVM-style CSE is not evidence that these operations may be
merged under our existing contract. This is a statement about that pass,
not proof that no modern compiler can improve this program.

The executable compiler experiment in `tools/references/fpcsex.c` is important:
Apple Clang 21 retains both strict additions in optimized IR but merges them
in final x87 code, while retaining SINGLE rounding stores/reloads. Therefore
the EarlyCSE check above must not become a blanket prohibition justified by
LLVM. Its backend behavior and exception-visibility differences need auditing;
see `tools/references/README.md`. The old reassociated, unrounded target is
still invalid, but useful code-generation opportunities demonstrably remain.

The current PDS stage dump (`/tmp/qbopt-fpcsex-strict-current`) retains both
additions in both CSE rounds. Emission still evaluates, in order:

```asm
fld  dword [a]
fadd dword [b]
fmul dword [c]
fstp dword [p]       ; SINGLE conversion
fld  dword [a]
fadd dword [b]
fdiv dword [c]
fstp dword [q]       ; SINGLE conversion
fld  dword [s]
fadd dword [p]
fadd dword [q]
fstp dword [s]       ; SINGLE conversion
wait
```

Next steps are to derive and validate a strict reference with these conversions
and observable exception ordering, and separately improve allocation or prove
specific operations exception-free. Do not turn on relaxed FP, erase conversion
points, or change the denominator merely to make this row pass. The inspection
changes no emitted code: PDS remains 985 object bytes and cost 4462.

Current PDS scores (2026-09-09): FPCSE **3956**, down from **4386** before
floating CSE; FPCSEX **4508**, unchanged. The reduction is 430 model units,
about 9.8%, not a measured hardware latency improvement. Both denominators
remain provisional.

**Semantic audit (2026-09-09): the proposed listing below is not yet a
valid strict-floating-point target.** It changes `(s + p) + q` into
`(p + q) + s` and retains intermediates beyond their SINGLE storage
rounding points. Neither transformation is generally valid without a
separate proof or an explicit relaxed-FP contract. For example, with
exactly representable SINGLE inputs `s=2^65`, `p=-2^65`, `q=1`, 64-bit
significand arithmetic with round-to-nearest/ties-even yields 1 in the
source order and 0 in the proposed order. This is an arithmetic
counterexample, not a claim about the runtime's configured precision.

Keep the existing numeric target visible as provisional, not as proof of
completion or permission to reassociate. A replacement derivation must
preserve source evaluation order and explicitly represent SINGLE rounding
at the p, q, and s stores. Raising needs floating SSA identities and those
rounding operations; MIR passes must not manage x87 stack positions.
Lowering owns their eventual stack placement and necessary memory round trips.

The previous section said BC's float code keeps its intermediates on the
x87 stack. That is true *within one statement*, and it is the only place it
is true. Give a subexpression to two statements and use both results in a
third:

```
p = (a + b) * c
q = (a + b) / c
s = s + p + q
```

```
0054  fld  [a]        | ; the x87 stack is eight deep
0059  fadd [b]        | fld  [a]
005e  fmul [c]        | fadd [b]        ; t = a+b, once
0063  fstp [p]        | fld  st(0)
0068  wait            | fmul [c]        ; p
006a  fld  [a]   <--  | fxch st(1)
006f  fadd [b]   <--  | fdiv [c]        ; q
0074  fdiv [c]        | fst  [q]
0079  fstp [q]        | fadd st(1)      ; p + q
007e  wait            | fadd [s]
0080  fld  [p]   <--  | fstp [s]
0085  fadd [q]   <--  | fxch / fstp [p]
008a  fstp [s]        |
008f  wait            |
```

`(a + b)` is computed twice. `p` and `q` are each stored and reloaded one
statement later. There is a `wait` per statement. **BC uses two of the
stack's eight slots** and goes through memory at every statement boundary --
which is the same behaviour as its integer code, for the same reason, and
costs more here because the values are four bytes and the operations are
tens of cycles.

In a ten-trip loop that is 4,448 against 1,340: **4.1x**, and the counters
show three loads of a cell just written and three of a cell already loaded,
per pass.

So the correction to the section below: BC's float code is not better than
its integer code. It is better *inside a statement*, and a statement is
exactly as far as BC ever looks.

## Floating point inside one statement

Worth recording because it went the other way, and see the section above
for how far it goes. BC's default is `/FPi`, so
every float operation is an `int 34h`..`3Dh` -- but the emulator patches
those sites to the real opcodes at load when a coprocessor is present, so
the cost is a 387's and not a software emulation's. Pricing them at 150
cycles was wrong and is fixed.

And the code itself is *better* than its integer code. For
`r = r + (a * b + c * d) / (a + b)`:

```
0072  fld  [a]      0086  faddp
0077  fmul [b]      0089  fld  [a]
007c  fld  [c]      008e  fadd [b]
0081  fmul [d]      0093  fdivp
                    0096  fadd [r]
                    009b  fstp [r]
```

Intermediates stay on the x87 stack; nothing is spilled to a temporary.
That is more than BC manages with integers. What is wrong with it is what is
wrong everywhere else: the whole expression is loop-invariant and it runs
ten times, reloading all six operands from memory on every pass.

## Why these read 3x to 7x and real programs are worse

With the trip counts read off each program's own bounds and applied to both
sides, the suite measures **1.3x to 16.7x**. The two at the top are the only
two whose loop body calls the runtime on every pass: `lngmix` at 16.7x is
two long divisions per iteration, and `huge` at 13x is two `B$HARY` calls
for one element.

That is the shape of it. BC's integer code is bad by a factor of three to
seven -- no allocation, no invariants hoisted, every address recomputed.
Where it hands the work to the runtime it is bad by a factor of fifteen,
because the helper does in software what the machine does in one
instruction.

**And the suite is mostly integer, so it understates real programs.** BC's
default is `/FPi`: every floating-point operation is an `int 34h`..`3Dh`
into the emulator, done in software. `fpemu` and `fpdeep` together carry 65
of them. A cost model reading mnemonics prices the most expensive thing in
the program at two, because the instruction is a two-byte interrupt -- which
is the same mistake as the flat rate for a call, in a third place.

So the honest reading of "twenty to fifty times" is: it is what a program
dominated by these calls costs, and every real BASIC program is -- floating
point, strings, dynamic arrays, long arithmetic. The integer loops here are
the part BC does *least* badly.

## The scoreboard

Every target below is hand-derived from the full listing, both sides: BC's
own body costed instruction by instruction, and the optimal listing costed
the same way. Nothing here is scaled from another program's savings, which
four of them were -- and every one of those four was too generous, by
between 1.5 and 2.3 times. Estimating the target flattered BC.

```
  program       cost  target  ratio   redundancy left
  lngmix        3504     210  16.7x    0
  huge          3944     304  13.0x    0
  harr         12454    1834   6.8x   18
  spill         7458    1122   6.6x    8
  nested        4794     768   6.2x   14
  press         1788     308   5.8x   11
  matrix       31970    6210   5.1x   12
  hotlop        1472     312   4.7x    6
  segld        30570    6704   4.6x   12
  split         1196     276   4.3x    9
  stride        2116     602   3.5x    5
  addrm         2426     754   3.2x   12
  ivchan        1843     560   3.3x    4
  rotate         826     294   2.8x    9
  arridx        1600     660   2.4x    5
  bools          210     126   1.7x   13
  subexp         203     162   1.3x    2
```

**BC runs between 1.3 and 16.7 times the cost of code written by hand**, and
the section above says why the top of that range is where the runtime is
called and why the suite understates real programs.

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

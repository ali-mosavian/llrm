# qbext -- 386 code for compiled BASIC

QB 4.5, PDS 7.1 and VBDOS. Compiled mode only.

## The goal

BC compiles every long operation into 16-bit halves through memory, and
calls the runtime for the rest. `f = a + b * c - d` becomes nine
instructions, a far call and a store nobody reads:

    66 FF 36 <c>      push  dword [c]
    66 FF 36 <b>      push  dword [b]
    9A <B$MUI4>       call  far              -- 40 to 66 cycles
    03 06 <a>         add   ax,[a]
    13 16 <a+2>       adc   dx,[a+2]
    2B 06 <d>         sub   ax,[d]
    1B 16 <d+2>       sbb   dx,[d+2]
    A3 <f>            mov   [f],ax
    89 16 <f+2>       mov   [f+2],dx                     38 bytes

Collapse a **run** of long operations into one 32-bit load, a chain of
32-bit operations, one 32-bit store:

    66 A1 <b>         mov   eax,[b]
    66 0F AF 06 <c>   imul  eax,[c]
    66 03 06 <a>      add   eax,[a]
    66 2B 06 <d>      sub   eax,[d]
    66 A3 <f>         mov   [f],eax                      24 bytes

The prize is the call and the traffic between operations, not the width of
any one of them. Correctness needs liveness -- which registers and flags
does anything still want -- and register allocation for the harder
expressions.

## The acceptance test, and where it does not apply

**A long must cost what an integer costs.** Measured, the same
expressions in both types:

    long / integer = 2.17x  as BC compiles it
                     1.28x  after the pass

That is DOSBox, which charges per instruction and so answers for an
in-order machine. tools/qbe/price.py asks tools/cycles instead:

    speedup         486     P5     P6     K5     K6     K7   Core
    alone         1.72x  1.81x  0.89x  1.03x  1.03x  1.03x  0.96x
    back to back  1.72x  1.81x  1.07x  1.74x  1.75x  1.75x  1.22x

BC's two halves are independent chains, one through `ax` and one through
`dx`, and an out-of-order machine already runs them together. Widening
puts everything through one register: it halves the instruction count and
leaves the chain where it was. So the win is whole on the in-order parts
and depends on surrounding work everywhere else.

which is exactly the halving -- BC does every operation twice. Widened,
the two are the same instruction against the same memory, one prefix byte
apart:

    LONG today                     INTEGER          widened LONG
    mov cx,[X]  mov bx,[X+2]       mov cx,[X]       mov ecx,[X]
    and cx,[Y]  and bx,[Y+2]       and cx,[Y]       and ecx,[Y]
    or  cx,[Z]  or  bx,[Z+2]       or  cx,[Z]       or  ecx,[Z]
    mov dx,bx   mov ax,cx
    mov [W],ax  mov [W+2],dx       mov [W],cx       mov [W],ecx
    ---------------------------    --------------   ---------------
    10 instructions                4                4

So parity is reachable for these, and 1.0x is the target. Longs are
avoided in mgl because they cost twice; the point of this is to make that
reason go away, which is why counting how much long arithmetic existing
code contains measures the wrong thing.

**Multiply and divide are different and parity is not reachable for
them.** Absorbing the call gets the instruction count to exactly what the
integer form uses:

    integer     mov ax,[b]  / imul word [c] / mov [p],ax        7 instructions
                mov ax,[b]  / cwd / idiv word [c] / mov [q],ax     for two
                                                                   statements
    long        mov eax,[b] / imul eax,[c]   / mov [p],eax      7 instructions
                mov eax,[b] / cdq / idiv dword [c] / mov [q],eax

and it still costs more, because a 32-bit multiply and divide are slower
instructions than the 16-bit ones:

    long/int, both inline   486    P5    P6    K5    K6    K7  Core
                           1.9x  1.7x  1.9x  1.7x  1.7x  1.6x  1.2x

That is the machine dividing twice as many bits, not code left on the
table. The reachable target for multiply and divide is the inline form,
and it is reached; the number to hold it to is against a call to the
runtime, which it beats by three to four times.

## Absorbing the call

`f = a * b` calls the runtime. That call and everything it runs become two
instructions:

    push dword [b] / push dword [a] / call far B$MUI4
    mov eax,[a] / imul eax,[b]

                              ins   486    P5    P6    K5    K6    K7  Core
    call + B$MUI4 fast path    14    68    34    42    11    12    14    43
    call + B$MUI4 full path    22   103    62    45    15    15    19    45
    mov eax,[a] / imul eax,[b]  2    30    14    10     8     7    10    10

    against the fast path          2.3x  2.4x  4.2x  1.4x  1.7x  1.4x  4.3x
    against the full path          3.4x  4.4x  4.5x  1.9x  2.1x  1.9x  4.5x

This is the one transformation whose worth does not turn on what a `66h`
prefix costs. Everything else here trades instruction count for prefixes,
and on an in-order part those may cancel; this removes a far call and the
routine behind it.

Divide and remainder too. Neither is commutative, so the operand order was
read off what BC emits rather than assumed: the first long pushed is the
divisor and the second the dividend, which is the one `B$DVI4` reads at
`[bp+6]`.

    mov eax,[a] / cdq / idiv dword [b]          the quotient
    ...and mov eax,edx after it                 the remainder

Both push shapes are matched, because the compilers disagree:

    VBDOS /G3    66 FF 36 <b>  66 FF 36 <a>  9A          15 bytes
    PDS, QB 4.5  FF 36 <b+2> FF 36 <b> FF 36 <a+2> FF 36 <a> 9A   21

The word form has room the dword form does not: `MOD` needs `mov eax,edx`
after the `idiv` and is three bytes longer than a divide, which fits 21
bytes with a fixup still to pay for and does not fit 15.

## Two halves

    qbeBoost  replaces the callee    B$MUI4, B$DVI4, B$RMI4, B$CPI4
    qbe       rewrites the caller    the push/call and the halves around it

## The qbe pipeline

Not a pattern matcher. Recover what the code *means* as 32-bit operations,
work out how the values flow, then emit the best sequence for that -- which
is not the same as translating each pair.

      BC's code, in memory
              |
              v
      +---------------------+  256-byte opcode table plus a 0F map.
      |  decode             |  Unknown opcode -> abandon the block
      |  qbe$len            |  rather than guess.
      +---------------------+
              |
              v
      +---------------------+  Ends at a jump, a return or an indirect
      |  find the block     |  jump. NOT at a call -- a call comes back.
      +---------------------+
              |
              v
      +---------------------+  THE UNDERSTANDING STEP. A pair of 16-bit
      |  lift               |  operations on ax:dx or cx:bx is one 32-bit
      |  16-bit pairs       |  operation on one value. A pair-to-pair copy
      |  -> 32-bit values   |  is a move. A store then a reload of the
      +---------------------+  same slot is the same value.
              |
              v
        +-----------+  values and the operations between them: a DAG per
        |  the DAG  |  block, with memory as the leaves
        +-----------+
              |
              v
      +---------------------+  Which values escape the block, which flags
      |  liveness           |  anything still wants, which halves are read
      +---------------------+  by something that was not lifted.
              |
              v
      +---------------------+  Values to the four 32-bit registers.
      |  allocate           |  Choosing well is what removes the copies
      +---------------------+  and the reloads.
              |
              v
      +---------------------+  Writes over the front of the block and
      |  emit               |  jumps over the slack. Nothing moves.
      +---------------------+

The lift is the part that makes the rest possible, and it is the part that
is only half built: the classifier recognises the pair idiom instruction by
instruction but stops there instead of producing values.

What that buys over translating each pair one for one:

    as BC emits it              1:1 widening        understood + rewritten
    --------------------------  ------------------  ----------------------
    mov cx,[X]  mov bx,[X+2]    mov ecx,[X]         mov ecx,[X]
    and cx,[Y]  and bx,[Y+2]    and ecx,[Y]         and ecx,[Y]
    or  cx,[Z]  or  bx,[Z+2]    or  ecx,[Z]         or  ecx,[Z]
    mov dx,bx   mov ax,cx       mov eax,ecx         (copy is dead)
    mov [W],ax  mov [W+2],dx    mov [W],eax         mov [W],ecx
    add ax,[W]  adc dx,[W+2]    add eax,[W]         add ecx,ecx
    xor ax,[V]  xor dx,[V+2]    xor eax,[V]         xor ecx,[V]
    mov [U],ax  mov [U+2],dx    mov [U],eax         mov [U],ecx
    --------------------------  ------------------  ----------------------
    16 instructions             8                   6

One for one reaches parity with integer. Understanding the dataflow goes
past it -- the copy is dead, and the reload of `[W]` is a value already in
a register.

## What the lift recognises

      +--------------------------+
      |  K_LDAX  mov ax,[X]      | \
      |  K_LDDX  mov dx,[X+2]    | /  ->  66 A1 X        4 bytes
      |  K_OPAX  and ax,[Y]      | \
      |  K_OPDX  and dx,[Y+2]    | /  ->  66 23 06 Y     5 bytes
      |  K_STAX  mov [Z],ax      | \      ... per pair
      |  K_STDX  mov [Z+2],dx    | /  ->  66 A3 Z        4 bytes
      +--------------------------+

Each pair must agree twice: the halves name the low then the high register
of one pair, and the second displacement is exactly two past the first. The
second test is what makes this one long rather than a coincidence about two
variables. The same reading applies to a pair-to-pair copy (`mov dx,bx` /
`mov ax,cx`) and to a pair consumed register to register (`xor ax,cx` /
`xor dx,bx`), which are a move and an operation between values.

Runs only shrink, so nothing moves:

      before  | ld lo | ld hi | op lo | op hi | st lo | st hi |
              |<------------------ 22 bytes ------------------>|
      after   | mov eax | and eax | mov [Z] | jmp +7 | nop x7 |
              |<---- 13 bytes ---->|<------ 9 bytes slack ---->|

A jump over the slack, not a run of nops -- a nop is an instruction. What
is jumped over is nopped anyway, so a later pass finds no wreckage there.

## Layout

    qbemath.asm    qbeBoost, CRACK trampolines, power-of-two shifts,
                   the divide/mod stub pool
    qbedec.asm     qbe$len, the instruction length decoder
    qbedtab.inc    its opcode tables    ) generated by
    qbedeq.inc     the flag equates     ) tools/qbe/gentab.py
    qbeopt.asm     block finder, IR, liveness, matcher, emitter
    qbewide.asm    superseded, see PLAN.md

`tools/qbe` holds a second implementation of the decoder and matcher in
Python, checked against the assembly on the same programs. That is how
nearly every bug here was found.

## Status

| | |
|---|---|
| length decoder | 5888 instructions, every length agreeing with ndisasm |
| block finder | calls fall through, indirect jumps end a block |
| liveness | `ax`, `dx`, flags -- both gates mutation checked |
| emitter | works; verified against the same code unrewritten |
| lift | 16-bit pairs, pair copies, cross-pair operations, long negate |
| emission | from the value graph, region at a time |
| footprint | 5859 bytes, all code segment, nothing in DGROUP |

Measured: long against integer, 2.17x down to 1.28x, 49 instructions down
to 28. The remaining gap is the fixups that put `dx` and `bx` back, seven
bytes a region, and the idioms still unlifted -- each of which invalidates
the register tracking and costs everything after it until the next load.

`PLAN.md` is what remains. `AGENTS.md` is what bit us getting here.

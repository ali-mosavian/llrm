# Worked examples

Audience: an engineer reading a dump for the first time. Three small Nib functions go through the machine phases at `-m16 -O2`; each dump is the output of `ISEL_DUMP=1 llrm-nib -m16 -O2 -S FILE` (plus `-fno-inline-functions` for the second), with each stage's `═══` banner line dropped. Dump text is explained in [debugging](debugging.md#what-prints). `main` appears because the callee was inlined into it.

## A loop that fits

[`tests/fixtures/nib/sum.nib`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/tests/fixtures/nib/sum.nib), the sum over a six-element array. Input of `RegAlloc` (the `Coalescer` dump):

```lir
fn main() -> @eax   ; calls clobber eax ecx edx ebx es fs gs
frame  slot0 [bp-12] 12 B

B0  entry  → B1                                                   ×1.0
     #1               store.i32   [slot0], 131073
     #2               store.i32   [slot0+4], 262147
     #3               store.i32   [slot0+8], 393221
     #4  t4         = lea.i16     &slot0
     #6  total.11   = mov.i16     0
     #7  t2         = mov.i16     -12
     #8               jmp         B1

    B1  ← B0 B1  → B1 B2  loop 6 trips                                ×6.0
        #11  t9         = load.i16    [ss:t4+t2+0xc]
        #12  total.11   = add.i16     total.11, t9
        #13  t2         = add.i16     t2, 2
        #14               cmp.i16     t2, 0
        #15               jne         B1 else B2                            ; 97% taken

B2  ← B1  → B3 B4                                                 ×1.0
    #17               cmp.i16     total.11, 21
    #18               je          B3 else B4                            ; 50% taken

B3  ← B2  → B5                                                    ×0.5
    #19  t12        = mov.i16     &seg0+0x6
    #20               call        far N$PS(t12@ax)
    #21               call        far N$PN()
    #22               jmp         B5

B4  ← B2  → B5                                                    ×0.5
    #23  t11        = mov.i16     &seg1+0x6
    #24               call        far N$PS(t11@ax)
    #25               call        far N$PN()
    #26               jmp         B5

B5  ← B3 B4  → exit                                               ×1.0
    #27               ret         total.11@eax
```

`total.11` is the running sum, `t2` the counter that steps to zero and `t4` the array's address (`lea`). The phi copies are gone: the coalescer joined each phi with its latch value. The calls to the runtime state what they need and destroy: the argument `t12@ax`, and the `fn` line's `calls clobber eax ecx edx ebx es fs gs`. Output of `RegAlloc`:

```lir
fn main() -> @eax   ; calls clobber eax ecx edx ebx es fs gs
frame  slot0 [bp-12] 12 B

B0  entry  → B1                                                   ×1.0
     #1               store.i32   [slot0], 131073
     #2               store.i32   [slot0+4], 262147
     #3               store.i32   [slot0+8], 393221
     #4  si         = lea.i16     &slot0
     #6  di         = mov.i16     0
     #7  bx         = mov.i16     -12
     #8               jmp         B1

    B1  ← B0 B1  → B1 B2  loop 6 trips                                ×6.0
        #11  ax         = load.i16    [ss:t4@bx+t2@si+0xc]
        #12  di         = add.i16     di, ax
        #13  bx         = add.i16     bx, 2
        #14               cmp.i16     bx, 0
        #15               jne         B1 else B2                            ; 97% taken

B2  ← B1  → B3 B4                                                 ×1.0
    #17               cmp.i16     di, 21
    #18               je          B3 else B4                            ; 50% taken

B3  ← B2  → B5                                                    ×0.5
    #28  ax         = mov.i16     &seg0+0x6
    #20               call        far N$PS(t13@ax)
    #21               call        far N$PN()
    #22               jmp         B5

B4  ← B2  → B5                                                    ×0.5
    #29  ax         = mov.i16     &seg1+0x6
    #24               call        far N$PS(t14@ax)
    #25               call        far N$PN()
    #26               jmp         B5

B5  ← B3 B4  → exit                                               ×1.0
    #30  ax         = mov.i16     di
    #27               ret         t15@eax
```

Nothing spilled. `t4` is in `si`, `t2` in `bx` and the sum `total.11` in `di`; the load reads `[bx+si+12]` (`t4@bx` names the cell's base register as placed). The sum is live across the calls, which destroy `bx` and the other registers listed, so it took `di`, which they leave. The call arguments `t13` and `t14` are in `ax`, as required.

## Constraints, ties and copies

Source (`-fno-inline-functions`):

```
fn mix(values: &[i16]) -> i16:
    let mut total: i16 = 1
    for value in &values:
        total += (total << value) + (value // total)
    return total

fn main() -> i16:
    let values: i16[4] = [1, 2, 3, 4]
    return mix(&values)
```

`TwoAddress` output: the phi of the counter is now a parallel copy (`; copy`, `lsr.iv` in `B5` and at the latch in `B3`), and each update is tied (`#43 lsr.iv.next = mov.i16 lsr.iv`, then `#26 lsr.iv.next = add.i16 lsr.iv.next, 2`):

```lir
fn main() -> @eax   ; calls clobber eax ecx edx ebx es fs gs
frame  slot0 [bp-8] 8 B

B0  entry  → B5                                                   ×1.0
     #1               store.i32   [slot0], 131073
     #2               store.i32   [slot0+4], 262147
     #3  t4         = lea.i16     &slot0
     #4  t3         = mov.i16     t4
     #5               jmp         B5

    B1  ← B3 B5  → B3 B4  loop                                        ×32.0
         #8  t9         = load.i16    [ss:t3+lsr.iv+0x8]
         #9               cmp.i16     t9, 16
        #10               jb          B3 else B4                            ; 100% taken

B2  ← B6  → exit                                                  ×1.0
    #12               ret         total.2.lcssa@eax

    B3  ← B1  → B1 B6                                                 ×32.0
        #38  total.11   = mov.i16     total.11
        #13  total.11   = shl.i16     total.11, t9:i8
        #14  t13        = cwd.i16     t9
        #15  t12, t11   = idiv.i16    t13, t9, total.11
        #16               cmp.i16     t11, 0
        #17  t14        = setne.i8
        #39  t15        = mov.i16     t11
        #18  t15        = xor.i16     t15, total.11
        #19               cmp.i16     t15, 0
        #20  t16        = setl.i8
        #40  t17        = mov.i8      t14
        #21  t17        = and.i8      t17, t16
        #22  t18        = movzx.i16   t17:i8
        #41  t19        = mov.i16     t12
        #23  t19        = sub.i16     t19, t18
        #42  total.11   = mov.i16     total.11
        #24  total.11   = add.i16     total.11, t19
        #25  total.11   = add.i16     total.11, total.11
        #43  lsr.iv.next = mov.i16     lsr.iv
        #26  lsr.iv.next = add.i16     lsr.iv.next, 2
        #27               cmp.i16     lsr.iv.next, 0
        #35  lsr.iv     = mov.i16     lsr.iv.next                           ; copy
        #28               jne         B1 else B6                            ; 97% taken

B4  ← B1  → exit  cold                                            ×0.0
    #29               call        far N$ESHF()

B5  ← B0  → B1                                                    ×1.0
    #30  t1         = mov.i16     -8
    #31  total.11   = mov.i16     1
    #36  lsr.iv     = mov.i16     t1                                    ; copy
    #32               jmp         B1

B6  ← B3  → B2                                                    ×1.0
    #37  total.2.lcssa = mov.i16     total.11
    #34               jmp         B2
```

`RegAlloc` input, after the coalescer:

```lir
fn main() -> @eax   ; calls clobber eax ecx edx ebx es fs gs
frame  slot0 [bp-8] 8 B

B0  entry  → B5                                                   ×1.0
     #1               store.i32   [slot0], 131073
     #2               store.i32   [slot0+4], 262147
     #3  t4         = lea.i16     &slot0
     #5               jmp         B5

    B1  ← B3 B5  → B3 B4  loop                                        ×32.0
         #8  t9         = load.i16    [ss:t4+t1+0x8]
         #9               cmp.i16     t9, 16
        #10               jb          B3 else B4                            ; 100% taken

B2  ← B6  → exit                                                  ×1.0
    #12               ret         total.11@eax

    B3  ← B1  → B1 B6                                                 ×32.0
        #38  total.11   = mov.i16     total.11
        #13  total.11   = shl.i16     total.11, t9:i8
        #14  t13        = cwd.i16     t9
        #15  t12, t11   = idiv.i16    t13, t9, total.11
        #16               cmp.i16     t11, 0
        #17  t14        = setne.i8
        #18  t11        = xor.i16     t11, total.11
        #19               cmp.i16     t11, 0
        #20  t16        = setl.i8
        #21  t14        = and.i8      t14, t16
        #22  t18        = movzx.i16   t14:i8
        #23  t12        = sub.i16     t12, t18
        #24  total.11   = add.i16     total.11, t12
        #25  total.11   = add.i16     total.11, total.11
        #26  t1         = add.i16     t1, 2
        #27               cmp.i16     t1, 0
        #28               jne         B1 else B6                            ; 97% taken

B4  ← B1  → exit  cold                                            ×0.0
    #29               call        far N$ESHF()

B5  ← B0  → B1                                                    ×1.0
    #30  t1         = mov.i16     -8
    #31  total.11   = mov.i16     1
    #32               jmp         B1

B6  ← B3  → B2                                                    ×1.0
    #34               jmp         B2
```

Output of `RegAlloc`:

```lir
fn main() -> @eax   ; calls clobber eax ecx edx ebx es fs gs
frame  slot0 [bp-8] 8 B

B0  entry  → B5                                                   ×1.0
     #1               store.i32   [slot0], 131073
     #2               store.i32   [slot0+4], 262147
     #5               jmp         B5

    B1  ← B3 B5  → B3 B4  loop                                        ×32.0
        #39  si         = lea.i16     &slot0                                ; remat
         #8  ax         = load.i16    [ss:t26@bx+t1@si+0x8]
         #9               cmp.i16     ax, 16
        #10               jb          B3 else B4                            ; 100% taken

B2  ← B6  → exit                                                  ×1.0
    #40  ax         = mov.i16     di
    #12               ret         t19@eax

    B3  ← B1  → B1 B6                                                 ×32.0
        #38  si         = mov.i16     di
        #41  cl         = mov.i8      al
        #13  si         = shl.i16     si, cl
        #42  total.2.lcssa = elided      t9
        #14  dx         = cwd.i16     ax
        #43  t13        = elided      total.1.lcssa
        #44  t25        = elided      t13
        #15  ax, dx     = idiv.i16    dx, ax, di
        #45  t11        = elided      t23
        #46  t12        = elided      t24
        #16               cmp.i16     dx, 0
        #17  cl         = setne.i8
        #18  dx         = xor.i16     dx, di
        #19               cmp.i16     dx, 0
        #20  dl         = setl.i8
        #21  cl         = and.i8      cl, dl
        #22  cx         = movzx.i16   cl
        #23  ax         = sub.i16     ax, cx
        #24  si         = add.i16     si, ax
        #25  di         = add.i16     di, si
        #26  bx         = add.i16     bx, 2
        #27               cmp.i16     bx, 0
        #28               jne         B1 else B6                            ; 97% taken

B4  ← B1  → exit  cold                                            ×0.0
    #29               call        far N$ESHF()

B5  ← B0  → B1                                                    ×1.0
    #30  bx         = mov.i16     -8
    #31  di         = mov.i16     1
    #32               jmp         B1

B6  ← B3  → B2                                                    ×1.0
    #34               jmp         B2
```

* `shl` counts in `cl`: `cl = mov.i8 al` copies the loaded element, which `ax` holds, into the count register before `si = shl.i16 si, cl`. This is the `s1=cx` of the `shl` form.
* `cwd` reads `ax` and writes `dx`; `idiv` reads `dx:ax` and writes the quotient to `ax` and the remainder to `dx`: `d0=ax,d1=dx,s0=dx,s1=ax`.
* `setne` and `setl` write a byte, so their values are in `cl` and `dl`, the byte class; `cx = movzx.i16 cl` widens the result.
* The array's address `t4` was made again by `lea` where the loop reads it (`si = lea.i16 &slot0`, noted `remat`): no `lea` is left in `B0`.
* The `elided` lines are operations selection removed that still carry a value; they emit no code.

## A loop with more values than registers

Source: seven running sums over an array. `-m16` has six allocatable registers, and the loop also needs the array pointer and a counter.

```
fn mix(values: &[i16]) -> i16:
    let mut a: i16 = 0
    let mut b: i16 = 1
    let mut c: i16 = 2
    let mut d: i16 = 3
    let mut e: i16 = 4
    let mut f: i16 = 5
    let mut g: i16 = 6
    for value in &values:
        a += value
        b += a
        c += b
        d += c
        e += d
        f += e
        g += f
    return a + b + c + d + e + f + g

fn main() -> i16:
    let values: i16[6] = [1, 2, 3, 4, 5, 6]
    return mix(&values)
```

`LLRM_DEBUG=regalloc,spill,candidates,cost` printed:

```
[spill] main: spill 10 with {10}
[spill] main: spill 20 with {20}
[spill] main: spill 14 with {14}
[spill] main: spill 24 with {24}
[spill] main: spill 34 with {34}
[regalloc] main: 41 insns, 5 spilled, cost 164, 0 forced
[regalloc]   trial Scoped (splitting true): 5 spilled, cost 187, 0 forced
[candidates] main: spiller Some(215.0) / Some(128.0), allocator alone Some(149.0) / Some(105.0)
[cost] main executes 82 instructions, 63 memory operands; 33 reloads, 3 spill stores, 6 remats; 6 branches, 6 taken, 0 jumps, 18 jump cycles
```

The base allocation spilled five values (`10`, `20`, `14`, `24`, `34`). The shape `Scoped` was tried and cost more, so it was not kept. The SSA-spill route was made too and its `route_cost` (215) is above the allocator alone's (149), so the allocator's own result is the one that follows. Its input, after the coalescer:

```lir
fn main() -> @eax
frame  slot0 [bp-12] 12 B

B0  entry  → B1                                                   ×1.0
     #1               store.i32   [slot0], 131073
     #2               store.i32   [slot0+4], 262147
     #3               store.i32   [slot0+8], 393221
     #4  t10        = lea.i16     &slot0
     #6  a.11       = mov.i16     0
     #7  b.11       = mov.i16     1
     #8  c.11       = mov.i16     2
     #9  d.11       = mov.i16     3
    #10  e.11       = mov.i16     4
    #11  f.11       = mov.i16     5
    #12  g.11       = mov.i16     6
    #13  t8         = mov.i16     -12
    #14               jmp         B1

    B1  ← B0 B1  → B1 B2  loop 6 trips                                ×6.0
        #23  t27        = load.i16    [ss:t10+t8+0xc]
        #24  a.11       = add.i16     a.11, t27
        #25  b.11       = add.i16     b.11, a.11
        #26  c.11       = add.i16     c.11, b.11
        #27  d.11       = add.i16     d.11, c.11
        #28  e.11       = add.i16     e.11, d.11
        #29  f.11       = add.i16     f.11, e.11
        #30  g.11       = add.i16     g.11, f.11
        #31  t8         = add.i16     t8, 2
        #32               cmp.i16     t8, 0
        #33               jne         B1 else B2                            ; 97% taken

B2  ← B1  → exit                                                  ×1.0
    #50  a.1.lcssa  = mov.i16     a.11
    #51  b.1.lcssa  = mov.i16     b.11
    #52  c.1.lcssa  = mov.i16     c.11
    #53  d.1.lcssa  = mov.i16     d.11
    #54  e.1.lcssa  = mov.i16     e.11
    #55  f.1.lcssa  = mov.i16     f.11
    #56  g.1.lcssa  = mov.i16     g.11
    #41  a.1.lcssa  = add.i16     a.1.lcssa, b.1.lcssa
    #42  a.1.lcssa  = add.i16     a.1.lcssa, c.1.lcssa
    #43  a.1.lcssa  = add.i16     a.1.lcssa, d.1.lcssa
    #44  a.1.lcssa  = add.i16     a.1.lcssa, e.1.lcssa
    #45  a.1.lcssa  = add.i16     a.1.lcssa, f.1.lcssa
    #46  a.1.lcssa  = add.i16     a.1.lcssa, g.1.lcssa
    #47               ret         a.1.lcssa@eax
```

and its output:

```lir
fn main() -> @eax
frame  slot0 [bp-12] 12 B  slot1 [bp-14] 2 B  slot2 [bp-16] 2 B  slot3 [bp-18] 2 B

B0  entry  → B1                                                   ×1.0
     #1               store.i32   [slot0], 131073
     #2               store.i32   [slot0+4], 262147
     #3               store.i32   [slot0+8], 393221
     #6  ax         = mov.i16     0
     #7  bx         = mov.i16     1
    #56               store.i16   [slot2], bx                           ; spill
     #8  dx         = mov.i16     2
     #9  di         = mov.i16     3
    #10  bx         = mov.i16     4
    #57               store.i16   [slot1], bx                           ; spill
    #11  cx         = mov.i16     5
    #12  bx         = mov.i16     6
    #58               store.i16   [slot3], bx                           ; spill
    #13  bx         = mov.i16     -12
    #14               jmp         B1

    B1  ← B0 B1  → B1 B2  loop 6 trips                                ×6.0
        #59  si         = lea.i16     &slot0                                ; remat
        #23  si         = load.i16    [ss:a.1.lcssa@bx+t8@si+0xc]
        #24  ax         = add.i16     ax, si
        #25               add.i16     [slot2], ax
        #26  dx         = add.i16     dx, [slot2]
        #27  di         = add.i16     di, dx
        #28               add.i16     [slot1], di
        #29  cx         = add.i16     cx, [slot1]
        #30               add.i16     [slot3], cx
        #31  bx         = add.i16     bx, 2
        #32               cmp.i16     bx, 0
        #33               jne         B1 else B2                            ; 97% taken

B2  ← B1  → exit                                                  ×1.0
    #50  a.1.lcssa  = elided      a.11
    #51  bx         = load.i16    [slot2]
    #52  c.1.lcssa  = elided      c.11
    #53  d.1.lcssa  = elided      d.11
    #54  si         = load.i16    [slot1]
    #55  f.1.lcssa  = elided      f.11
    #41  ax         = add.i16     ax, bx
    #42  ax         = add.i16     ax, dx
    #43  ax         = add.i16     ax, di
    #44  ax         = add.i16     ax, si
    #45  ax         = add.i16     ax, cx
    #46  ax         = add.i16     ax, [slot3]
    #60  a.1.lcssa  = elided      a.1.lcssa
    #47               ret         a.1.lcssa@eax
```

* `t10`, the array's frame address, was spilled and remade: `si = lea.i16 &slot0` is in the loop, before the read, noted `remat`, with no slot and no store.
* `b.11`, `e.11` and `g.11` have `slot2` (`[bp-16]`), `slot1` (`[bp-14]`) and `slot3` (`[bp-18]`). Their initial values are stored once in the first block (`store.i16 [slot2], bx ; spill`) and the loop updates them in memory: `add.i16 [slot2], ax` takes the slot as the memory operand, with no reload and no store of its own.
* The exit block loads `[slot2]` into `bx`, `[slot1]` into `si`, and uses `[slot3]` as the memory operand of the last `add`.
* The `cost` line is `executed`: 33 reloads, 3 spill stores and 6 remats, each counted by the expected executions of its block.

The code after `LoopSlots`, `Peephole` and layout:

```
L0_0:
    mov dword ptr [bp-12], 131073
    mov dword ptr [bp-8], 262147
    mov dword ptr [bp-4], 393221
    xor ax, ax
    mov bx, 1
    mov word ptr [bp-16], bx
    mov dx, 2
    mov di, 3
    mov bx, 4
    mov word ptr [bp-14], bx
    mov cx, 5
    mov bx, 6
    mov word ptr [bp-18], bx
    mov bx, -12
L0_9:
    lea si, [bp-12]
    add ax, word ptr ss:[bx+si+12]
    add word ptr [bp-16], ax
    add dx, word ptr [bp-16]
    add di, dx
    add word ptr [bp-14], di
    add cx, word ptr [bp-14]
    add word ptr [bp-18], cx
    add bx, 2
    jne L0_9
L0_30:
    mov si, word ptr [bp-14]
    add ax, word ptr [bp-16]
    add ax, dx
    add ax, di
    add ax, si
    add ax, cx
    add ax, word ptr [bp-18]
    pop di
    pop si
    leave
    retf
```

# Worked examples

Audience: an engineer reading a dump for the first time. Three small Nib functions go through the machine phases at `-m16 -O2`; each dump is the output of `ISEL_DUMP=1 llrm-nib -m16 -O2 -S FILE` (plus `-fno-inline-functions` for the second), with the `; inserted` note, on every line, dropped, and instructions that print nothing dropped. Dump text is explained in [debugging](debugging.md#what-prints). `main` appears because the callee was inlined into it.

## A loop that fits

[`tests/fixtures/nib/sum.nib`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/tests/fixtures/nib/sum.nib), the sum over a six-element array. Input of `RegAlloc` (the `Coalescer` dump):

```
  main: 23 instructions
    block 0x0000
      0x0002  [[bp-0xc]] := mov 0x20001
      0x0004  [[bp-0x8]] := mov 0x40003
      0x0006  [[bp-0x4]] := mov 0x60005
      0x0007  v4 := lea Address(addr=[bp-0xc], through=26, index=0, scale=1, offset=0, disp_width=2)
      0x0008  v6 := mov 0x0
      0x0008  v2 := mov -12
      0x0008  jmp
    block 0x0009
      0x000d  v9 := mov [[abs+0xc] v4@unplaced]
      0x000e  v6 := add v6, v9
      0x000f  v2 := add v2, 0x2
      0x0011  cmp v2, 0x0
      0x0011  jne
    block 0x0012
      0x0014  cmp v6, 0x15
      0x0014  je
    block 0x0015
      0x0016  v12 := mov 0x0
      0x0016  call   ; requires v12@AX; clobbers EAX,ECX,EDX,EBX,ES,FS,GS
      0x0017  call   ; clobbers EAX,ECX,EDX,EBX,ES,FS,GS
      0x0018  jmp
    block 0x0019
      0x001a  v11 := mov 0x0
      0x001a  call   ; requires v11@AX; clobbers EAX,ECX,EDX,EBX,ES,FS,GS
      0x001b  call   ; clobbers EAX,ECX,EDX,EBX,ES,FS,GS
      0x001c  jmp
    block 0x001d
      0x001d     ; requires v6@EAX
```

`v6` is the running sum, `v2` the counter that steps to zero and `v4` the array's address (`lea`). The phi copies are gone: the coalescer joined each phi with its latch value. The calls to the runtime state what they need and destroy: `requires v12@AX`, `clobbers EAX,ECX,EDX,EBX,ES,FS,GS`. Output of `RegAlloc`:

```
  main: 24 instructions
    block 0x0000
      0x0002  [[bp-0xc]] := mov 0x20001
      0x0004  [[bp-0x8]] := mov 0x40003
      0x0006  [[bp-0x4]] := mov 0x60005
      0x0007  SI := lea Address(addr=[bp-0xc], through=26, index=0, scale=1, offset=0, disp_width=2)
      0x0008  DI := mov 0x0
      0x0008  BX := mov -12
      0x0008  jmp
    block 0x0009
      0x000d  AX := mov [[abs+0xc] v4@BX]
      0x000e  DI := add DI, AX
      0x000f  BX := add BX, 0x2
      0x0011  cmp BX, 0x0
      0x0011  jne
    block 0x0012
      0x0014  cmp DI, 0x15
      0x0014  je
    block 0x0015
      0x0016  AX := mov 0x0
      0x0016  call   ; requires v13@AX; clobbers EAX,ECX,EDX,EBX,ES,FS,GS
      0x0017  call   ; clobbers EAX,ECX,EDX,EBX,ES,FS,GS
      0x0018  jmp
    block 0x0019
      0x001a  AX := mov 0x0
      0x001a  call   ; requires v14@AX; clobbers EAX,ECX,EDX,EBX,ES,FS,GS
      0x001b  call   ; clobbers EAX,ECX,EDX,EBX,ES,FS,GS
      0x001c  jmp
    block 0x001d
      0x001d  AX := mov DI
      0x001d     ; requires v15@EAX
```

Nothing spilled. `v4` is in `SI`, `v2` in `BX` and the sum `v6` in `DI`; the load reads `[bx+si+12]` (`v4@BX` names the cell's base register as placed). The sum is live across the calls, which destroy `BX` and the other registers listed, so it took `DI`, which they leave. The pinned `v12` and `v11` are in `AX`, as required.

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

`TwoAddress` output: the phis are now parallel copies (`parallel-copy 1` before the loop and `2` at the latch), and each update is tied (`v11 := mov v8`, then `v11 := shl v11, v10`):

```
  main: 39 instructions
    block 0x0000
      0x0002  [[bp-0x8]] := mov 0x20001
      0x0004  [[bp-0x4]] := mov 0x40003
      0x0005  v4 := lea Address(addr=[bp-0x8], through=26, index=0, scale=1, offset=0, disp_width=2)
      0x0005  v3 := mov v4
      0x0006  v1 := mov -8
      0x0006  v2 := mov 0x1
      0x0006  v6 := mov v1  ; parallel-copy 1
      0x0006  v8 := mov v2  ; parallel-copy 1
      0x0006  jmp
    block 0x0007
      0x000a  cmp v6, 0x0
      0x000a  jne
    block 0x000b
      0x000d  v10 := mov [[abs+0x8] v3@unplaced]
      0x000f  cmp v10, 0x10
      0x000f  jb
    block 0x0010
      0x0011  v9 := mov v8
      0x0011     ; requires v9@EAX
    block 0x0012
      0x0012  v11 := mov v8
      0x0012  v11 := shl v11, v10
      0x0013  v14 := cwd v10
      0x0013  v13, v12 := idiv v14, v10, v8
      0x0015  cmp v12, 0x0
      0x0015  v15 := setne
      0x0016  v16 := mov v12
      0x0016  v16 := xor v16, v8
      0x0017  cmp v16, 0x0
      0x0017  v17 := setl
      0x0018  v18 := mov v15
      0x0018  v18 := and v18, v17
      0x0019  v19 := movzx v18
      0x001a  v20 := mov v13
      0x001a  v20 := sub v20, v19
      0x001b  v21 := mov v11
      0x001b  v21 := add v21, v20
      0x001c  v8 := add v8, v21
      0x001d  v6 := add v6, 0x2
      0x001e  v6 := mov v6  ; parallel-copy 2
      0x001e  v8 := mov v8  ; parallel-copy 2
      0x001e  jmp
    block 0x001f
      0x001f  call   ; clobbers EAX,ECX,EDX,EBX,ES,FS,GS
```

`RegAlloc` input, after the coalescer:

```
  main: 29 instructions
    block 0x0000
      0x0002  [[bp-0x8]] := mov 0x20001
      0x0004  [[bp-0x4]] := mov 0x40003
      0x0005  v4 := lea Address(addr=[bp-0x8], through=26, index=0, scale=1, offset=0, disp_width=2)
      0x0006  v1 := mov -8
      0x0006  v2 := mov 0x1
      0x0006  jmp
    block 0x0007
      0x000a  cmp v1, 0x0
      0x000a  jne
    block 0x000b
      0x000d  v10 := mov [[abs+0x8] v4@unplaced]
      0x000f  cmp v10, 0x10
      0x000f  jb
    block 0x0010
      0x0011     ; requires v2@EAX
    block 0x0012
      0x0012  v11 := mov v2
      0x0012  v11 := shl v11, v10
      0x0013  v14 := cwd v10
      0x0013  v13, v12 := idiv v14, v10, v2
      0x0015  cmp v12, 0x0
      0x0015  v15 := setne
      0x0016  v12 := xor v12, v2
      0x0017  cmp v12, 0x0
      0x0017  v17 := setl
      0x0018  v15 := and v15, v17
      0x0019  v19 := movzx v15
      0x001a  v13 := sub v13, v19
      0x001b  v11 := add v11, v13
      0x001c  v2 := add v2, v11
      0x001d  v1 := add v1, 0x2
      0x001e  jmp
    block 0x001f
      0x001f  call   ; clobbers EAX,ECX,EDX,EBX,ES,FS,GS
```

Output of `RegAlloc`:

```
  main: 36 instructions
    block 0x0000
      0x0002  [[bp-0x8]] := mov 0x20001
      0x0004  [[bp-0x4]] := mov 0x40003
      0x0006  BX := mov -8
      0x0006  DI := mov 0x1
      0x0006  jmp
    block 0x0007
      0x000a  cmp BX, 0x0
      0x000a  jne
    block 0x000b
      0x000d  SI := lea Address(addr=[bp-0x8], through=26, index=0, scale=1, offset=0, disp_width=2)
      0x000d  AX := mov [[abs+0x8] v27@BX]
      0x000f  cmp AX, 0x10
      0x000f  jb
    block 0x0010
      0x0011  AX := mov DI
      0x0011     ; requires v20@EAX
    block 0x0012
      0x0012  SI := mov DI
      0x0012  CL := mov AL
      0x0012  SI := shl SI, CL
      0x0013  DX := cwd AX
      0x0013  AX, DX := idiv DX, AX, DI
      0x0015  cmp DX, 0x0
      0x0015  CL := setne
      0x0016  DX := xor DX, DI
      0x0017  cmp DX, 0x0
      0x0017  DL := setl
      0x0018  CL := and CL, DL
      0x0019  CX := movzx CL
      0x001a  AX := sub AX, CX
      0x001b  SI := add SI, AX
      0x001c  DI := add DI, SI
      0x001d  BX := add BX, 0x2
      0x001e  jmp
    block 0x001f
      0x001f  call   ; clobbers EAX,ECX,EDX,EBX,ES,FS,GS
```

* `shl` counts in `CL`: `CL := mov AL` copies the loaded element, which `AX` holds, into the count register before `SI := shl SI, CL`. This is the `s1=cx` of the `shl` form.
* `cwd` reads `AX` and writes `DX`; `idiv` reads `DX:AX` and writes the quotient to `AX` and the remainder to `DX`: `d0=ax,d1=dx,s0=dx,s1=ax`.
* `setne` and `setl` write a byte, so their values are in `CL` and `DL`, the byte class; `movzx CX, CL` widens the result.
* The array's address `v4` was made again by `lea` where the loop reads it (`SI := lea ...`): no `lea` is left in the first block.

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

```
  main: 38 instructions
    block 0x0000
      0x0002  [[bp-0xc]] := mov 0x20001
      0x0004  [[bp-0x8]] := mov 0x40003
      0x0006  [[bp-0x4]] := mov 0x60005
      0x0007  v10 := lea Address(addr=[bp-0xc], through=26, index=0, scale=1, offset=0, disp_width=2)
      0x0008  v12 := mov 0x0
      0x0008  v14 := mov 0x1
      0x0008  v16 := mov 0x2
      0x0008  v18 := mov 0x3
      0x0008  v20 := mov 0x4
      0x0008  v22 := mov 0x5
      0x0008  v24 := mov 0x6
      0x0008  v8 := mov -12
      0x0008  jmp
    block 0x0009
      0x0013  v27 := mov [[abs+0xc] v10@unplaced]
      0x0014  v12 := add v12, v27
      0x0015  v14 := add v14, v12
      0x0016  v16 := add v16, v14
      0x0017  v18 := add v18, v16
      0x0018  v20 := add v20, v18
      0x0019  v22 := add v22, v20
      0x001a  v24 := add v24, v22
      0x001b  v8 := add v8, 0x2
      0x001d  cmp v8, 0x0
      0x001d  jne
    block 0x001e
      0x0025  v28 := mov v12
      0x0025  v29 := mov v14
      0x0025  v30 := mov v16
      0x0025  v31 := mov v18
      0x0025  v32 := mov v20
      0x0025  v33 := mov v22
      0x0025  v34 := mov v24
      0x0025  v28 := add v28, v29
      0x0026  v28 := add v28, v30
      0x0027  v28 := add v28, v31
      0x0028  v28 := add v28, v32
      0x0029  v28 := add v28, v33
      0x002a  v28 := add v28, v34
      0x002b     ; requires v28@EAX
```

and its output:

```
  main: 41 instructions
    block 0x0000
      0x0002  [[bp-0xc]] := mov 0x20001
      0x0004  [[bp-0x8]] := mov 0x40003
      0x0006  [[bp-0x4]] := mov 0x60005
      0x0008  AX := mov 0x0
      0x0008  BX := mov 0x1
      0x0008  [[bp-0x10]] := mov BX
      0x0008  DX := mov 0x2
      0x0008  DI := mov 0x3
      0x0008  BX := mov 0x4
      0x0008  [[bp-0xe]] := mov BX
      0x0008  CX := mov 0x5
      0x0008  BX := mov 0x6
      0x0008  [[bp-0x12]] := mov BX
      0x0008  BX := mov -12
      0x0008  jmp
    block 0x0009
      0x0013  SI := lea Address(addr=[bp-0xc], through=26, index=0, scale=1, offset=0, disp_width=2)
      0x0013  SI := mov [[abs+0xc] v36@BX]
      0x0014  AX := add AX, SI
      0x0015  [[bp-0x10]] := add [[bp-0x10]], AX
      0x0016  DX := add DX, [[bp-0x10]]
      0x0017  DI := add DI, DX
      0x0018  [[bp-0xe]] := add [[bp-0xe]], DI
      0x0019  CX := add CX, [[bp-0xe]]
      0x001a  [[bp-0x12]] := add [[bp-0x12]], CX
      0x001b  BX := add BX, 0x2
      0x001d  cmp BX, 0x0
      0x001d  jne
    block 0x001e
      0x0025  BX := mov [[bp-0x10]]
      0x0025  SI := mov [[bp-0xe]]
      0x0025  AX := add AX, BX
      0x0026  AX := add AX, DX
      0x0027  AX := add AX, DI
      0x0028  AX := add AX, SI
      0x0029  AX := add AX, CX
      0x002a  AX := add AX, [[bp-0x12]]
      0x002b     ; requires v35@EAX
```

* `v10`, the array's frame address, was spilled and remade: `SI := lea ...` is in the loop, before the read, with no slot and no store.
* `v14`, `v20` and `v24` have slots `[bp-0x10]`, `[bp-0xe]` and `[bp-0x12]`. Their initial values are stored once in the first block (`[[bp-0x10]] := mov BX`) and the loop updates them in memory: `add [[bp-0x10]], AX` takes the slot as the memory operand, with no reload and no store of its own.
* The exit block reads `[bp-0x10]` into `BX`, `[bp-0xe]` into `SI`, and `[bp-0x12]` as the memory operand of the last `add`.
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

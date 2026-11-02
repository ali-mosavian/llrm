# llrm

`llrm` is a cross compiler. Several source languages and several targets share one optimizer and one backend
framework. For every target the aim is to match or beat gcc and LLVM, or the best compiler for that target and
platform, on code size and speed, at a compile time near LLVM's.

The optimizer and backend are a Rust translation of LLVM, pinned to one commit. A frontend raises its language to one
SSA IR (MIR); target-neutral passes run on it; a target is a set of description files (registers, instructions, calling
conventions, costs) the shared backend reads. No shared pass names a target. Documentation starts at
[docs](docs/readme.md); [comparison](docs/comparison/readme.md) sets the output beside other compilers'.

![llrm pipeline: QuickBASIC, Nib and C raise to MIR; one target-neutral optimizer; one backend per target](docs/architecture/pipeline.svg)

## Targets

| Target | Platform, object | Compared against |
|---|---|---|
| x86 16-bit | DOS real mode, OMF | Borland, Watcom, BC, gcc, clang |
| x86 32-bit | DOS under an extender (OMF), Linux (ELF) | gcc, clang |
| x86-64 | Linux (ELF) | gcc, clang |
| arm64 | Linux (ELF), macOS (Mach-O) | gcc, clang |
| 6502, 65C02 | ca65 assembly, sim65 | llvm-mos, cc65, SDCC, hand-written assembly |
| m68k (68000, 68020) | assembly | gcc, clang |
| Z80, WebAssembly | in development | SDCC; clang (wasm32) |

The gate refuses a change that makes any benchmark cell worse than main's. What is measured, and against which
version of each compiler, is in [comparison](docs/comparison/readme.md).

## Programs

| Program | Does |
|---|---|
| `llrm-cc` | `cc`-compatible driver: C and C++ through the pinned clang, then llrm's backend; `-target NAME` picks the target from [targets.toml](crates/driver/llrm-cc/targets.toml) |
| `llrm-qb` | QuickBASIC-family source (QB 4.5, PDS 7.1, VBDOS) to an object |
| `llrm-nib` | Nib, llrm's own language, to an object |
| `llrm-c` | C through Open Watcom's front end, DOS targets only |
| `llrm-omf` | Objects written by QuickBASIC's BC, raised to MIR and compiled afresh |
| `llrm-mir` | Reads, verifies and runs MIR text |

## Build and use

```sh
cargo build --release
target/release/llrm-cc -target arm64-linux -O2 -S examples/dot/dot.c
```

The default feature `toolchain` bootstraps Open Watcom, `jwasm`, `jwlink` and the headless DOSBox-X the DOS tests run
on. `llrm-cc` needs the pinned clang (`toolchain/clang/pin`; [how it runs](docs/frontends/c/clang-route.md)). The
options are gcc's: `-O0`..`-O3`, `-Os`, `-Oz`, `-f[no-]PASS`, `-march`, `-g`, `-S`; see [levels](docs/reference/levels.md).
The Rust toolchain is pinned by `rust-toolchain.toml`; format with `tools/fmt.sh`; tests are in
[testing](docs/working/testing.md).

## Example

[`examples/dot`](examples/dot): a dot product of two arrays, returned as a `long`, compiled with `llrm-cc -O2 -S`.

```c
long dot(const int *a, const int *b, int n)
{
    long total = 0;
    int i;
    for (i = 0; i < n; i++)
        total += (long)a[i] * b[i];
    return total;
}
```

x86-64 (`-target x86_64-linux`):

```asm
dot:
    test edx, edx
    jle  L2
    movzx rcx, edx
    xor  eax, eax
    xor  edx, edx
L3: movsx r8, dword ptr [rdi+rdx*4]
    movsx r9, dword ptr [rsi+rdx*4]
    imul r9, r8
    add  rax, r9
    inc  rdx
    cmp  rcx, rdx
    jne  L3
    ret
L2: xor  eax, eax
    ret
```

arm64 (`-target arm64-linux`):

```asm
dot:
    orr  x11, xzr, x0, lsl #0
    subs wzr, w2, #0
    b.le .Lzero
    movz x0, #0, lsl #0
.Lloop:
    ldrsw x10, [x11]
    ldrsw x9, [x1]
    madd x0, x9, x10, x0
    sub  x2, x2, #1
    cbz  x2, .Ldone
    add  x11, x11, #4
    add  x1, x1, #4
    b    .Lloop
.Ldone:
    ret  x30
.Lzero:
    movz x0, #0, lsl #0
    ret  x30
```

m68k, 68020 (`-target m68k-68020`):

```asm
dot:
    movem.l %d2-%d3, -(%sp)
    move.l  (20,%sp), %d2
    ble     .Lzero
    move.l  (16,%sp), %a0
    move.l  (12,%sp), %a1
    moveq   #0, %d0
.Lloop:
    move.l  (%a1)+, %d3
    move.l  (%a0)+, %d1
    muls.l  %d3, %d1
    add.l   %d0, %d1
    move.l  %d1, %d0
    subq.l  #1, %d2
    bne     .Lloop
    bra     .Ldone
.Lzero:
    moveq   #0, %d0
.Ldone:
    movem.l (%sp)+, %d2-%d3
    rts
```

x86 16-bit, medium model, `int` is 16 bits (`-target x86_16-dos`):

```asm
_dot proc far
    push bp
    mov  bp, sp
    push si
    push di
    mov  cx, word ptr [bp+10]       ; n
    test cx, cx
    jle  zero
    mov  di, word ptr [bp+6]        ; a
    xor  eax, eax
    xor  bx, bx
loop:
    movsx edx, word ptr [bx+di]
    mov  si, word ptr [bp+8]        ; b, reloaded every iteration
    movsx esi, word ptr [bx+si]
    imul esi, edx
    add  eax, esi
    add  bx, 2
    dec  cx
    jne  loop
    jmp  done
zero:
    xor  eax, eax
done:
    shld edx, eax, 16               ; the long returns in DX:AX
    pop  di
    pop  si
    pop  bp
    retf
_dot endp
```

The listings are `llrm-cc -O2 -S` output at the commit that wrote this section; labels are shortened and comments added.

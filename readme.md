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

x86-64, `-target x86_64-linux`:

```asm
dot proc near
L0_1:
    test edx, edx
    jle L0_2_1
L0_2:
    movzx rcx, edx
    xor eax, eax
    xor edx, edx
L0_3:
    movsx r8, dword ptr [rdi+rdx*4]
    movsx r9, dword ptr [rsi+rdx*4]
    imul r9, r8
    add rax, r9
    inc rdx
    cmp rcx, rdx
    jne L0_3
L0_4:
    ret
L0_2_1:
    xor eax, eax
    ret
dot endp
```

arm64, `-target arm64-linux`:

```asm
dot:
.Ldot.0:
	orr x11, xzr, x0, lsl #0
	subs wzr, w2, #0
	b.le .Ldot.e4
.Ldot.1:
	movz x0, #0, lsl #0
.Ldot.2:
	ldrsw x10, [x11]
	ldrsw x9, [x1]
	madd x0, x9, x10, x0
	sub x2, x2, #1
	cbz x2, .Ldot.e5
.Ldot.e6:
	add x11, x11, #4
	add x1, x1, #4
	b .Ldot.2
.Ldot.e5:
	ret x30
.Ldot.e4:
	movz x0, #0, lsl #0
	ret x30
```

m68k, 68020, `-target m68k-68020`:

```asm
dot:
	movem.l	%d2-%d3, -(%sp)
	move.l	(20,%sp), %d2
	ble	.LBB0_3
	move.l	(16,%sp), %a0
	move.l	(12,%sp), %a1
	moveq	#0, %d0
.LBB0_2:
	move.l	(%a1)+, %d3
	move.l	(%a0)+, %d1
	muls.l	%d3, %d1
	add.l	%d0, %d1
	move.l	%d1, %d0
	subq.l	#1, %d2
	bne	.LBB0_2
	bra	.LBB0_4
.LBB0_3:
	moveq	#0, %d0
.LBB0_4:
	movem.l	(%sp)+, %d2-%d3
	rts
```

x86 16-bit, medium model, `int` is 16 bits, `-target x86_16-dos`:

```asm
_dot proc far
    push bp
    mov bp, sp
    push si
    push di
L0_1:
    mov cx, word ptr [bp+10]
    test cx, cx
    jle L0_2_1
L0_2:
    mov di, word ptr [bp+6]
    xor eax, eax
    xor bx, bx
L0_3:
    movsx edx, word ptr [bx+di]
    mov si, word ptr [bp+8]
    movsx esi, word ptr [bx+si]
    imul esi, edx
    add eax, esi
    add bx, 2
    dec cx
    jne L0_3
    jmp L0_4
L0_2_1:
    xor eax, eax
L0_4:
    shld edx, eax, 16
    pop di
    pop si
    pop bp
    retf
_dot endp
```

The listings are `llrm-cc -O2 -S` output of the function, verbatim; `tests/test_readme.py` holds them to it.

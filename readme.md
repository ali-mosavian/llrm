# llrm -- Low Level Real Machine

`llrm` is a compiler suite for 16-bit real-mode DOS that aims at GCC/LLVM-quality
code. Every frontend raises to one SSA IR (MIR), shares one optimizer and x86
backend, and writes linkable OMF `.OBJ` files.

![llrm pipeline: QuickBASIC, Nib and C raise through HIR and BC objects straight to MIR; one target-neutral optimizer; one x86 real-mode backend](docs/architecture/pipeline.svg)

MIR passes are machine-independent; only lowering, allocation and peephole see
registers or instructions. See [the MIR boundary](docs/architecture/split.md). The goal is
output within 1.5× a hand-derived modern-compiler listing for every suite
program; [targets](docs/measurement/targets.md) has the evidence and current gaps.

## Frontends

| Tool | Input |
| --- | --- |
| `llrm-qb` | QuickBASIC-family source: QB 4.5, QBasic 1.1, PDS 7.1, VBDOS, and QuickrBASIC |
| `llrm-c` | C, through a patched Open Watcom front end (`toolchain/owshim/`), in Borland's medium model |
| `llrm-nib` | Nib, llrm's own language; see [the language](docs/frontends/nib/readme.md) |
| `llrm-omf` | OMF objects produced by QuickBASIC's BC, rewritten in place |
| `llrm-mir` | MIR text: verifies it and writes it back; `--run` executes it on the interpreter, `--ivs` counts each innermost loop's induction variables |
| `llrm-run` | Runs a Nib module's entry on the host HIR interpreter |
| `nib-lsp` | Nib's language server; see [the server](docs/frontends/nib/lsp.md) |

QuickrBASIC (`--dialect quickr`) is VBDOS BASIC extended, linked against the VBDOS
runtime. It adds sized and unsigned integers, mandatory declarations, f-strings,
PRIVATE procedures, `+=`, BREAK/CONTINUE, `FOR x IN`, `a IF c ELSE b`, IN, chained
comparisons, `RETURN value`, tuples, record and array results, and string slices
`s(a:b:c)`. See [the dialects](docs/frontends/qb/dialects.md).

`FOR x [AS type] IN …` walks a one-dimensional array (`a()`), `RANGE(stop)` or
`RANGE(start, stop[, step])`, the characters of a string, or the array a FUNCTION
`AS t()` returns. `x` is a copy of each element, and BREAK, CONTINUE and EXIT FOR
work as in any loop.

```basic
DIM q AS INTEGER, r AS INTEGER, parity AS STRING
q, r = divmod(17, 5)
FOR i IN RANGE(q)
    parity = "even" IF i MOD 2 = 0 ELSE "odd"
    PRINT f"{i}: {parity}"
NEXT

FUNCTION divmod (a AS INTEGER, b AS INTEGER) AS (INTEGER, INTEGER)
    RETURN a \ b, a MOD b
END FUNCTION
```

The compilers and `llrm-omf` tune with `-march`, 386 through Core. Floating point is native x87, so a
coprocessor is required.

## Use

`cargo build --release` builds everything from a fresh checkout, including `jwasm`, `jwlink`
and the headless DOSBox-X the e2e tests run on, in `target/release`. Their sources are cloned
under `~/.cache/llrm`, one tree per pinned commit, made whole or not at all, so worktrees and
parallel builds share them safely. Open Watcom's first bootstrap takes about three minutes; set
`$OWROOT` to use an existing tree. `--no-default-features --features wccq` builds only Open
Watcom's front end, which the lib tests need (CI does). On Debian or Ubuntu the build needs:

```sh
sudo apt install build-essential git autoconf automake libtool libpng-dev libpcap-dev libncurses-dev
```

```sh
cargo build --release
target/release/llrm-qb PROGRAM.BAS --dialect qb45 --runtime qb45 -o PROGRAM.OBJ
target/release/llrm-c program.c -o PROGRAM.OBJ
target/release/llrm-nib program.nib -o PROGRAM.OBJ
target/release/llrm-omf PROGRAM.OBJ -o PROGRAMQ.OBJ -march=i486
```

`llrm-qb`, `llrm-c` and `llrm-nib` share their options:

| Option | Does |
| --- | --- |
| `-O0` `-O1` `-O2` `-O3` `-Os` `-Oz` `-Og` | Optimization level; `-O2` is the default |
| `-f[no-]PASS` | One pass on or off, by gcc's name: `unroll-loops`, `peel-loops`, `inline-functions`, `strength-reduce`, `unswitch-loops`, `gcse`, `tree-dse`, `tree-dce`, `tree-sra`, `move-loop-invariants`, `tree-loop-distribute-patterns`, `optimize-sibling-calls` |
| `-march=CPU`, `-mtune=CPU` | The processor, gcc's name for it (`i386` through `core2`); `-m16`, `-m32` choose the target |
| `-fsanitize=bounds,integer-divide-by-zero,signed-integer-overflow,undefined`, `-ftrapv` | The run-time checks BC's `/D` makes, as gcc names them |
| `-fsanitize=stack` | Each function compares SP with its runtime's stack limit once its frame is allocated and calls the runtime's overflow routine out of line (BASIC: `b$pendchk`, `B$ERR_OSS`, as BC `/D`; C: Open Watcom's `_STACKLOW`, `__STKOVERFLOW`; Nib: `_llrm_os_stack_low`, `N$ESTK`). Not part of `undefined`. +8 bytes and 3 instructions per call, +5 bytes cold; a small leaf the runtime's red zone covers goes unchecked |
| `-g` | CodeView line numbers, symbols and types, for `LINK /CO` and CodeView |
| `-S` | Writes the assembly listing instead of an object |
| `--dump DIR` | Writes every stage to `DIR`, for diffing |

`llrm-qb` frames each procedure itself, without the runtime's `B$ENRA`/`B$EXSA`,
wherever the runtime needs no frame of its own. `--runtime-frames` ([Debug](#debug))
keeps the runtime's in every procedure.

`llrm-omf` takes every object and library in LINK order when a program spans
modules, and writes nothing unless all of them succeed:

```sh
target/release/llrm-omf MAIN.OBJ DRAW.OBJ BCOM45.LIB --output-dir build/optimized
LINK build/optimized/MAIN.OBJ+build/optimized/DRAW.OBJ,,,BCOM45.LIB
```

Unsupported records, unresolved externals and unencodable code are errors;
`--allow-unchanged` keeps a refused object as it was. `--basic-semantics` keeps
BASIC's numeric runtime errors and conversions, and `--bounds-checks` keeps
array checks. Audited external calls come from `--contracts PROFILE.json`; see
[the profile format](docs/qrender/contracts/readme.md).

## Optimizations

The middle end repeats its MIR passes to a fixed point: constant folding and
propagation, algebraic simplification, GVN over MemorySSA (which also forwards
loads and stores and reuses quotients), dead-store and dead-code removal,
promotion of memory to values, LICM, loop rotation, unswitching, unrolling and
peeling, induction variables and strength reduction, memset recognition,
inlining, and interprocedural facts: which calls read or write what, and which
never return.

The backend chooses x86 address forms, uses 32-bit registers and arithmetic in
real mode on the 386 and later, allocates registers with coalescing, live-range
splitting and spill placement, then runs machine CSE, copy propagation,
peephole, scheduling and jump layout. The BC frontend adds recognition of BC's
LONG register pairs, runtime arithmetic calls and array descriptors; see
[HARR](docs/optimizations/harr.md) for a before and after.

## Example

One loop in three languages: a dot product of two `int` arrays, returned as a
`long`. The sources are in [examples/dot](examples/dot); each listing is what `-S`
prints with `-march=i486`, and the comments are added by hand. C is in the medium model.

C, `llrm-c dot.c -march=i486 -S`:

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

```asm
_dot@3 proc far
    push si                         ; a is in ax, b in dx, n in cx (regparm3); si and di are callee-saved
    push di
L0_0:
    mov si, ax                      ; si = a, an address register: ax cannot address
    mov di, dx                      ; di = b
    lea ax, [ecx+ecx]               ; ax = 2n, the byte length of each array
    mov bx, ax
    neg bx                          ; [one induction variable] bx = -2n is the counter and the offset
    add si, ax                      ; [biased] a + 2n, so a[i] is at [bx+si]
    add di, ax                      ; [biased] b + 2n
    xor eax, eax                    ; total = 0
    or cx, cx
    jle L0_6                        ; [loop rotation] one guard for n <= 0; the test moves to the bottom
L0_21:
L0_8:
    movsx ecx, word ptr [bx+si]     ; a[i], sign-extended to 32 bits
    movsx edx, word ptr [bx+di]     ; b[i]
    imul ecx, edx                   ; (long)a[i] * b[i]: a 32-bit multiply, no runtime helper
    add eax, ecx                    ; total += product
    add bx, 2                       ; step one int; [flag reuse] its flags end the loop at 0
    jne L0_8
L0_6:
    pop di                          ; the long is already in eax: it returns there
    pop si                          ; no frame: nothing is on the stack
    retf
_dot@3 endp
```

Nib, `llrm-nib dot.nib --entry dot -march=i486 -S`:

```
fn dot(a: &[i16], b: &[i16]) -> i32:
    let mut total: i32 = 0
    for (x, y) in zip(a, b):
        total += i32(x) * i32(y)
    return total
```

```asm
_dot proc far
    push bp                         ; a is the far pointer [bp+6], b [bp+10]
    mov bp, sp
    push si
    push di
L0_0:
    les bx, dword ptr [bp+6]        ; es:bx = a's slice: length, then data pointer
    lfs di, dword ptr [bp+10]       ; fs:di = b's slice
    mov ax, word ptr es:[bx]        ; len(a)
    mov cx, word ptr fs:[di]        ; len(b)
    les si, dword ptr es:[bx+4]     ; [hoisted] es:si = a's data
    lfs di, dword ptr fs:[di+4]     ; [hoisted] fs:di = b's data
    cmp ax, cx
    mov bx, cx
    jae L0_9
L0_8:
    mov bx, ax
L0_9:                               ; bx = min(len(a), len(b)): zip ends with the shorter slice,
    lea ax, [ebx+ebx]               ; decided once, not tested in the loop
    mov cx, ax
    neg cx                          ; [one induction variable] cx = -2*count
    add si, ax                      ; [biased] a's data + 2*count
    add di, ax                      ; [biased] b's data + 2*count
    xor eax, eax                    ; total = 0
    or bx, bx
    je L0_29                        ; [loop rotation] no elements: skip the loop
L0_31:
    mov bx, cx
L0_16:
    movsx ecx, word ptr es:[bx+si]  ; a[i]
    movsx edx, word ptr fs:[bx+di]  ; b[i]
    imul ecx, edx
    add eax, ecx                    ; total += product
    add bx, 2                       ; [flag reuse] the step's flags end the loop at 0
    jne L0_16                       ; zip pairs the elements: no index, no bounds check
L0_29:
    pop di                          ; the long is in eax, where it returns
    pop si
    pop bp
    retf
_dot endp
```

A Nib slice is a far pointer to its length and data pointer. `zip` pairs the
elements until the shorter slice ends, so there is no `n`, and no index to check.
Indexing, `a[i]`, checks every access and calls `N$EBND` on a bad one.

BASIC, `llrm-qb dot.bas --dialect qb45 --runtime qb45 -march=i486 -O3 --whole-program -fno-inline-functions-called-once -S` (DOT is called once, and would go into its caller):

```basic
DECLARE FUNCTION Min% (BYVAL x AS INTEGER, BYVAL y AS INTEGER)
DECLARE FUNCTION Dot& (a() AS INTEGER, b() AS INTEGER)

n = 9: m = 7
REDIM p(n) AS INTEGER, q(m) AS INTEGER
PRINT Dot&(p(), q())

FUNCTION Min% (BYVAL x AS INTEGER, BYVAL y AS INTEGER)
    IF x < y THEN Min% = x ELSE Min% = y
END FUNCTION

FUNCTION Dot& (a() AS INTEGER, b() AS INTEGER)
    DIM total AS LONG, i AS INTEGER
    FOR i = 0 TO Min%(UBOUND(a), UBOUND(b))
        total = total + CLNG(a(i)) * b(i)
    NEXT
    Dot& = total
END FUNCTION
```

```asm
DOT proc near
    push bp                         ; a plain frame, not B$ENRA; near, as --whole-program
    mov bp, sp                      ; sees every caller. a() is [bp+6], b() [bp+4]
    push si
    push di
L1_0:
    mov di, word ptr [bp+6]         ; a() descriptor: the arguments were pushed left to right
    mov si, word ptr [bp+4]         ; b() descriptor
    mov bx, word ptr [di+14]        ; UBOUND(a): one dimension, so its slot is fixed, from 0,
    dec bx                          ; so the bound is its count less 1; no allocated test
    mov ax, word ptr [si+14]
    dec ax                          ; UBOUND(b)
    cmp bx, ax                      ; [inlined] Min%: no call, bx = the smaller bound
    jl L1_12
L1_11:
    mov bx, ax
L1_12:
    mov es, word ptr [di+2]         ; [hoisted] es = a's data segment
    mov ax, word ptr [di+10]        ; [hoisted] a's data offset
    mov fs, word ptr [si+2]         ; [hoisted] fs = b's data segment
    mov di, word ptr [si+10]        ; [hoisted] di = b's data offset
    lea dx, [ebx+ebx]               ; dx = 2 * bound
    mov cx, dx
    neg cx
    add cx, -2                      ; [one induction variable] cx = -2 * (bound + 1)
    lea si, [eax+edx]               ; [biased] a's offset + 2 * bound, one lea; the +2 is in the loop's
    add di, dx                      ; [biased] b's offset + 2 * bound; displacement
    xor eax, eax                    ; total = 0
    or bx, bx
    jl L1_49                        ; [loop rotation] bound < 0: no iterations
L1_51:
    mov bx, cx
L1_34:
    movsx ecx, word ptr es:[bx+si+2] ; a(i), sign-extended
    movsx edx, word ptr fs:[bx+di+2] ; b(i)
    imul ecx, edx                   ; CLNG(a(i)) * b(i)
    add eax, ecx                    ; total += product
    add bx, 2                       ; [flag reuse] the step's flags end the loop at 0
    jne L1_34
L1_49:
    shld edx, eax, 16               ; the long returns in DX:AX
    pop di
    pop si
    pop bp
    ret 4                           ; the callee pops the two arguments
DOT endp
```

An array is a descriptor: its segment goes in `es` or `fs` before the loop.
`UBOUND` is two loads and a decrement: the rank is the declaration's, so the count's
slot is fixed, and nothing tests that the array is allocated, as BC does not without
`/D`. `-fsanitize=bounds` brings the test and the cold `B$UBND` call back, except
where every caller passes an array a `DIM` or `REDIM` dominates, as here. Where a
procedure's own `DIM` or `REDIM` states a bound, `UBOUND` is that value and reads nothing.
The frame is a plain one, not the runtime's `B$ENRA` and `B$EXSA`.
`Min%` takes its arguments `BYVAL` and the build is `--whole-program`, which is what
lets the inliner take it; by reference it stays a call
([#114](https://github.com/ali-mosavian/llrm/issues/114)).

All three compile to the same loop: six instructions, one induction variable.
Loop strength reduction ([#98](https://github.com/ali-mosavian/llrm/issues/98))
biases each array's base by its byte length, so the counter, running from minus
that length up to zero, is also the offset of every access, and its flags end the
loop. The product is a 32-bit `imul` in `eax`, without a runtime call.

## Debug

`-g` makes an object carry CodeView line numbers, symbols and types. `LINK /CO`
and CVPACK accept it, and CodeView shows the source, locals, parameters and
`TYPE`s. [debugging.md](docs/debugging.md) finds a miscompile in a running DOS
program with dosrun: break on write, stack traces and map-file symbols.

`llrm-qb --runtime-frames` calls the runtime's frame entry and exit in every
procedure, so its frame chain, stack check and event poll are there to debug
against. `-g` does not imply it: CodeView's local offsets match either frame.
`--own-frames` is accepted and does nothing.

## Validate

```sh
cargo test --release <name>
uv run --project tools python tools/loops/run.py --quick
```

The second command runs the loop corpus: one loop program per case, written once
and emitted as C, QuickBASIC and Nib. An independent oracle checks each result,
in llrm-mir's interpreter and on DOSBox. Each inner loop's induction variables,
reloads and size are measured from the object's bytes, against bounds derived
from the target and against gcc-ia16 and Open Watcom. `tools/loops/known.toml`
lists what falls short today; a run fails for a shortfall not listed and for a
listed one that is gone. Without `--quick` it covers every case, CPU and `-O`
level, in about 23 minutes. [tools/readme.md](tools/readme.md) lists the other
tools.

For every failure, dump every stage and diff the first changed pair. Every fix
needs a regression that fails before the fix. Testing details are in
[docs/testing.md](docs/testing.md).

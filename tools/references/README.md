# Compiler reference kernels

These are independent compiler comparisons, not executable DOS targets and
not replacements for the complete listings in `docs/targets.md`.

## FPCSEX

`fpcsex.c` preserves extended intermediate arithmetic, source-ordered sums,
and assignment conversion to SINGLE after p, q and sum. The three output
objects are distinct, as in the BASIC program; `restrict` prevents invented
aliasing from impairing the reference. Inputs are runtime arguments, not DATA
constants. The caller supplies the initial sum (zero for the suite program).

Generate optimized IR and x87 assembly with an available Clang:

```sh
clang -target i386-unknown-linux-gnu -march=i386 -mno-sse -mno-sse2 \
  -O2 -ffp-model=strict -fno-pic -S -emit-llvm \
  tools/references/fpcsex.c -o /tmp/qbopt-fpcsex-reference.ll
clang -target i386-unknown-linux-gnu -march=i386 -mno-sse -mno-sse2 \
  -O2 -ffp-model=strict -fno-pic -S -masm=intel \
  tools/references/fpcsex.c -o /tmp/qbopt-fpcsex-reference.s
```

Record `clang --version`: the installed compiler need not match the local
LLVM source checkout. Inspect constrained intrinsic exception and rounding
metadata in the IR, not just the command-line flags.

This uses a 32-bit C ABI, not the 16-bit BASIC runtime ABI. It does not model
input/print calls, observable BASIC loop-counter state, emulator patch sites,
or resumable BASIC exceptions. C long-double evaluation also does not by
itself establish behavior under every x87 precision-control setting. These
differences must be resolved before deriving a whole-program denominator.
The kernel is evidence about compiler instruction selection, not proof of
semantic equivalence to the DOS program or measured hardware performance.

### Observed comparison

Apple Clang 21.0.0 (`clang-2100.1.1.101`) with the commands above retains
four strict constrained additions in optimized IR, including both a+b
computations. Its final x87 loop has three additions: one a+b and the two
source-ordered accumulator additions. It holds a, b and c across iterations,
and preserves all three SINGLE rounding points with scratch stores/reloads.
The public output stores are sunk after the loop. This last behavior is
another reason not to transplant the listing into BASIC without checking
exception-handler memory visibility.

Representative arithmetic, omitting stack housekeeping:

```asm
; qbopt loop                    ; compiled reference loop
fld  dword [a]                  fld st(1)
fadd dword [b]                  fadd st,st(3)     ; a+b once
fmul dword [c]                  fld st(0)
fstp dword [p]                  fmul st,st(5)
fld  dword [a]                  fstp dword [scratchP]
fadd dword [b]                  fld dword [scratchP]
fdiv dword [c]                  fxch st(1)
fstp dword [q]                  fdiv st,st(5)
                                fstp dword [scratchQ]
                                fld dword [scratchQ]
```

Do not infer full legality from compiler output alone: the reason its backend
merges the strict operations still needs tracing. But rejecting the optimization
based only on EarlyCSE would also be wrong. The immediate engineering gaps
are cross-block floating allocation (currently explicitly refused by
`backend/floatalloc.py`) and a justified FP reuse rule. No full-program score
is derived from this kernel, and qbopt's own before/after output is unchanged.

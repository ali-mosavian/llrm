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

### Trap-enabled limitation found in the source

At local LLVM revision `338e0c94943a6fb917c276bbbd9ff4b6cd6dd71e`,
`llvm/lib/CodeGen/SelectionDAG/SelectionDAGBuilder.cpp` explains the difference:

- `getFPOperationRoot`, lines 1187–1201, allows strict operations between
  barriers to share a root when exceptions are observed through status flags.
  Its comment requires source ordering when traps are enabled, then explicitly
  leaves support for that scenario as a TODO.
- `pushFPOpOutChain`, lines 8667–8687, defers root updates and groups pending
  chains with a TokenFactor, treating them as independent.
- `visitConstrainedFPIntrinsic`, lines 8691–8703, uses that shared root.

The installed Clang's before/after `machine-cse` dumps already contain a single
`ADD_Fp80` for a+b **before** MachineCSE. That pass is not the source of the
merge; in the local source it also explicitly rejects `mayRaiseFPException()`.
The shared-DAG-root behavior explains how identical strict operations can
be shared earlier, but the installed Apple compiler revision is different:
this is a source-supported explanation, not a trace of its exact build.

Consequently, do not adopt this reference's operation reuse as proof for
trap-enabled BASIC. Reuse needs an exception-free operation proof or a verified
masked-exception region with no intervening environment observation/change.
Cross-block allocation must separately retain conversion and synchronization
semantics. The allocator now supports owned 80-bit cross-region storage and
parallel floating phi transfers, including critical edges. That removes the
allocation refusal, not the need to prove reuse legality. Neither changing
the global strict contract nor accepting the old 1340 denominator follows
from this experiment.

# Compiler reference kernels

These are independent compiler comparisons, not executable DOS targets and
not replacements for the complete listings in `docs/targets.md`.

## Fixed-point square root

`fixed_sqrt.py` is an independent value experiment for an unsigned fixed-point
square root. It compares a normalized table seed plus exactly two integer
Newton divisions and exact rounding against an exact `isqrt` oracle and an
actual x87 `FILD`/`FSQRT`/`FISTP` helper. On Apple Silicon the helper is built
as x86_64 and run through Rosetta; other non-x86 hosts use a clearly labelled
binary64 fallback unless `--require-x87` is supplied.

```sh
uv run python -m tools.references.fixed_sqrt --require-x87
uv run python -m tools.references.fixed_sqrt --matrix --require-x87
```

Matrix mode writes `fixed-sqrt-matrix.svg` and `fixed-sqrt-errors.svg`; use
`--plot PATH` and `--error-plot PATH` to select different destinations. The
first figure shows worst pre-correction distance and the share already at the
floor root. The error figure shows maximum and mean absolute percentage error
against the mathematical square root, the 99th-percentile correction distance,
and final correctly rounded error in output LSBs.

The default format is unsigned Q16.16. This experiment changes no compiler,
runtime, optimizer, or scoreboard code.

## FPDEEP PDS reference

`fpdeep.asm` is a hand-written JWasm listing retaining source-visible numeric
stores and pending-exception checks. `fpdeep.py` wraps its bytes in the
hash-pinned PDS fixture's original OMF data and runtime relocations; it does
not invoke llrm optimization or instruction selection.

```sh
uv run python -m tools.references.fpdeep /tmp/fpdeep-reference --assembler /path/to/jwasm
```

The build explicitly selects native FPU (`-FPi87`). It emits BASE.OBJ,
REF.OBJ, and `fpdeep-p-g2.obj` for raw scoring. The reference has 21 stores,
21 WAITs and the original output calls: 1317 modeled units. It links and
prints byte-identical output to BC and qbopt's native build; evidence is in
`/tmp/qbopt-fpdeep-jwasm.xq6rYz`, including `result.png`. The accepted scope
is ordinary PDS without events or resumable errors. See `docs/targets.md`
for the derivation; other compiler configurations remain provisional.

## FPCSEX

`fpcsex.asm` is an executable, native-x87 **candidate**, not yet the scoreboard
denominator. Build its audited PDS object with:

```sh
uv run python tools/references/reference.py /tmp/fpcsex-reference --program fpcsex
```

It keeps both additions, source-order accumulation, three SINGLE conversions
and checkpoints per iteration, counter stores and runtime input/output calls.
The counted loop rotates from a top test to a bottom test because its ten
iterations are unconditional; the final counter remains 11. JWasm output
links with PDS BCL71ENR and matches the original's `S= 487.5` / `DONE`.
Varied inputs, rounding/exception cases and an independent cost audit remain
required before adoption. The C experiment below is not its legality proof.

`fpcsex_check.py` builds a DOS COM comparison harness from the actual original
and candidate object kernels. Original emulator opcodes become same-length
WAIT/ESC instructions; branches retain their displacements. It compares p, q,
s, i and the x87 status word for 12 input triples × 12 control words (four
rounding modes, three precisions). Inputs include cancellation, subnormals,
overflow, zero division, signed zero and quiet/signaling NaNs.

```sh
uv run python tools/references/fpcsex_check.py /tmp/fpcsex-reference
# Run CHECK.COM in the guest, then:
uv run python tools/references/fpcsex_check.py /tmp/fpcsex-reference --check
```

The ARM DOSBox-X run matched all 144 pairs but **failed environment
qualification**: directed SINGLE rounding did not change results, and 1/0
did not set ZE. Its local `fpu_instructions.h` uses a host float cast in
`FPU_FST_F32` (with a rounding TODO) and plain host division in `FPU_FDIV`
(with a flags TODO). Pairwise equality here cannot validate x87 semantics.
The checker rejects this result rather than certifying it. A qualified x87
execution engine is still required; QEMU and Bochs are available locally.

QEMU 10.2.0 TCG (`-cpu pentium3`) subsequently passed qualification and all
144 comparisons. For the non-exact sample at 53-bit precision, downward and
upward SINGLE rounding produce p bits `3f666666` and `3f666667`; division by
zero sets ZE (`0004`), and overflow sets OE+PE (`0028`). These are measured
masked-exception results, not coverage of unmasked traps or BASIC handlers.

The bare-metal transport loads the same kernels and emits their result bytes
through QEMU's debug port, avoiding DOSBox's floating implementation:

```sh
uv run python tools/references/fpcsex_check.py /tmp/fpcsex-reference --qemu
qemu-system-i386 -accel tcg -cpu pentium3 -m 16 \
  -drive file=/tmp/fpcsex-reference/qemu.img,format=raw,if=floppy -boot a \
  -display none -serial none -monitor none \
  -debugcon file:/tmp/fpcsex-reference/QEMU.BIN -global isa-debugcon.iobase=0xe9 \
  -device isa-debug-exit,iobase=0xf4,iosize=0x04 -no-reboot
uv run python tools/references/fpcsex_check.py /tmp/fpcsex-reference --qemu --check
```

QEMU's expected exit code is 33 from the explicit completion port, not zero.
Incomplete output or failed environment qualification is rejected regardless
of pairwise agreement. No compiler pass or scoreboard denominator changes here.

Add `--traps` to both harness commands and send QEMU debug output to
`TRAPS.BIN` to include each of the six exception masks individually cleared.
The bare-metal #MF handler snapshots state and terminates that kernel call
at its first trap; it does not model BASIC RESUME or arbitrary user handlers.
All 216 cases pass on QEMU after restoring WAIT before each arithmetic
instruction, matching the original emulator protocol's WAIT/ESC sequence.
The earlier candidate stored an indefinite NaN into q after invalid 0/0;
the original trapped before that store. Removing these waits reproduces the
failure in `tests/test_fpcsex_traps.py` (about two seconds, including build).

```asm
; rejected candidate          ; corrected candidate
fdiv dword ptr [c]            wait
fstp dword ptr [q]            fdiv dword ptr [c]
wait                         wait
                             fstp dword ptr [q]
                             wait
```

Hand audit under the existing opportunity cost model:

| Candidate component | Cost |
|---|---:|
| 12 memory arithmetic/conversion instructions per iteration | 403 |
| 15 WAITs per iteration | 75 |
| counter store, increment, compare and branch per iteration | 12 |
| ten iterations | 4900 |
| input calls/setup, initialization, final counter and output | 258 |
| Total | 5158 |

The scorer independently returns 5158. Current native qbopt scores 4456;
its top test actually executes eleven times, adding 10 to a hand-counted
dynamic total of 4466 under that same model. These are ranking units, not
measured CPU cycles; unknown helper calls are charged the model's default 20.
This conservative candidate is **not an established modern-compiler optimum**
and does not replace the old provisional denominator merely because its cost
is higher. The native output also omits wait points retained by this strict
candidate; their semantic policy must match before judging a ratio.

```asm
; current qbopt                ; candidate reference
inc ax                        inc ax
mov [i],ax                    cmp ax,11
cmp ax,10                     jne loopBody ; stores i at loop entry
jle loopBody                  mov [i],ax   ; final 11
```

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
`src/backend/floatalloc.rs`) and a justified FP reuse rule. No full-program score
is derived from this kernel, and llrm's own before/after output is unchanged.

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

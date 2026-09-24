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

## Native-FPU PDS snapshot — 2026-09-11

**Refreshed after dispatch and LCSSA follow-ups:** 28 of 29 measured PDS
programs have comparable targets and are within 1.5x; FPCSEX is provisional.
Only JUMPS's object differs from the saved pre-dispatch scan: 1264 → 1058.
FPDEEP remains 1771 with its verified 1317 reference. All 29 emit through
LIR; this does not replace runtime or all-compiler validation. The complete
refresh is recorded in [target-coverage-current.md](target-coverage-current.md).

Historical pre-dispatch snapshot:

The pre-dispatch allocator/rematerialization worktree emitted 29
target-bearing `*-p-g2.obj` fixtures with `native_fpu=True`. The scorer read
those emitted files directly (`--raw`), not a second default rewrite.
Artifacts and the measurement script: `/tmp/qbopt-native-targets.Xr96r6`.
This is a modeled-cost snapshot, not runtime validation or all-compiler coverage.

- 26 comparable programs are within 1.5x.
- JUMPS remains above the goal: **1264 / 742 = 1.70x**.
- FPCSEX costs 4456; its reference remains provisional for the semantic
  reasons recorded below. FPDEEP's then-provisional 1771-unit result now has
  a complete, executed PDS reference: **1771/1317 = 1.34x** (see below).
- HG and FX have target entries but no matching input in this fixture
  selection; their coverage is missing, not passed.

The next measured optimization gap is JUMPS's multiway dispatch and
branched constant-trip loop. Fresh stage dumps still show `B$OGTA` as a CALL
after unrolling, and emitted code retains `mov bx,[bp-2] / call far B$OGTA`.
Recognition belongs in the raise, with the selector-range and outgoing-value
proofs below; bounded CFG unrolling then exposes each iteration to SCCP.
No dispatch optimization is claimed implemented by this measurement.
Subsequent guarded dispatch recognition initially emitted JUMPS at
**1366/742 = 1.84x**. Range-driven branch simplification now removes its
unreachable error call/table and enables register-resident loop counting:
**1058/742 = 1.43x**, with byte-identical PDS runtime output. This closes the
JUMPS miss in the snapshot; provisional/missing targets above remain open.
See `switches.md` for the runtime gate and before/after assembly.

Frontend prerequisite: ON GOTO now takes successors from its own validated
inline relocation fields, not every relocated code label in the module.
On `jumptable.obj`, the edge set changes from `46,52,5e,ea` to `46,52,5e`;
`ea` belongs to the statement table. Ordered and repeated destinations are
retained separately from the deduplicated CFG edges. Unproved tables keep
the conservative edges; the runtime call and its exceptional behavior remain.
The regression failed first and failed again with the fix disabled. Fourteen
focused dispatch cases pass, including QB/PDS/VBDOS and event-enabled output;
the preceding block check passed 1,977 cases.

All-stage dumps in `/tmp/qbopt-dispatch-edges-{before,after}-20260911`
show identical MIR and emitted ASM for this fixture: later reachability had
already discarded that non-code edge. This is a frontend CFG correction,
not a measured speedup or closure of JUMPS's target gap:

```asm
; before                         ; after
mov bx,2                         mov bx,2
call far B$OGTA                  call far B$OGTA
```

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
QB BOOLS at that audit was **172 / 126 = 1.37x**. That instrument correction
changed no emitted assembly; the newer plain BOOLS reference is below.
Emission refusals remain UNMEASURED, not provisional successes or optimized
fallbacks. Event semantics and unsupported event paths still need work.

The fail-first instrument regressions cover renamed objects from QB, PDS
and VBDOS, plus a below-target cost that must still fail completion. No
denominator was increased or inferred from current output.

## BOOLS — constant evaluation with final stores retained

For event-free builds, `a=3`, `b=7`, `c=2`, `d=9`; BASIC's true value is
`-1`, so `x=-1` and `t=0-1+1+2=2`. No input, exception or runtime call
observes the intermediate assignments. Retain all six final INTEGER values
before the first print call, and retain the three printing calls and termination.
The adjacent pairs have the following little-endian representation:

```asm
mov dword [a], 00070003h       ; a=3, b=7
mov dword [c], 00090002h       ; c=2, d=9
mov dword [x], 0002FFFFh       ; x=-1, t=2
push offset textT
call B$PSSD
push 2
call B$PEI2
push offset textDone
call B$PESD
call B$CENP
```

Target: **116 modeled units** = three stores × (2+4), three immediate pushes
× (2+4 for the stack write), and four calls × 20. This replaces the older
126 reference, which retained an extra word store and a memory-reading push.
It is a ranking-model target, not measured processor clocks or DOSBox time.
Event-enabled builds remain provisional.

After CFG cleanup, backend store packing changes only:

```asm
; before                      ; after
mov word [x], -1               mov dword [x], 0002FFFFh
mov word [t], 2
```

Across QB/PDS/VBDOS plain fixtures: **122 → 116** modeled units. QB's emitted
code shrinks by three bytes; its object shrinks 736 → 726 bytes, including
the removed relocation record. Fail-first emitted-instruction regressions and
actual output checks cover all three compilers.

## FLAGS — constant branches with observable stores retained

For event-free builds, evaluate the source's integer expressions exactly:
65535 AND 61680 is 61680 (both SPLIT and FUSED are nonzero);
-65536 AND -65536 is -65536 (MIRROR is nonzero); 0 AND 0 is zero;
65536 - 1 is 65535 (SIGN is positive). None overflows or faults.
There is no input or error handler. Six fixed strings are printed, including
DONE. The runtime calls remain in source order, and every numeric assignment
is retained at its original position relative to those calls. No assumption
that a runtime call cannot observe the numeric globals is needed.

The complete hand-derived listing is:

```asm
mov dword [a],65535
mov dword [b],61680
mov dword [r],61680
push word splitNonzero
call far B$PESD
push word fusedNonzero
call far B$PESD
mov dword [a],-65536
mov dword [b],-65536
mov dword [r],-65536
push word mirrorNonzero
call far B$PESD
mov dword [a],0
mov dword [b],0
mov dword [r],0
push word bothZero
call far B$PESD
mov dword [a],65536
mov dword [b],1
mov dword [r],65535
push word signPositive
call far B$PESD
push word done
call far B$PESD
call far B$CENP
```

Using the same whole-program cost model on both sides: twelve stores cost
12*(2+4)=72; six descriptor pushes and print calls cost 6*(6+20)=156;
termination costs 20. **Target: 248.** It is derived from the source's four
observable three-value states and six output calls, not by scaling current
output. It makes no claim about hardware execution time. Event-enabled builds
still require a separate reference and remain provisional.

## FPCSE — complete reference with entry observation and final stores

The old 98-unit output-only reference omitted observable memory and pending
floating exceptions. It is replaced by a complete, source-derived listing,
not scaled from optimized output. Compiler identity matters: the raw PDS and
VBDOS objects initialize a, b, c, s and i before their first floating load;
QB starts with a floating load before any initialization. The first load's
pending-exception observation must remain in that position. Existing
`test_checkpoint_keeps_initial_memory_and_counter_stores` covers this distinction.

For the ordinary, event-free constant-input program, `a=2`, `b=4`, `c=8`:
each iteration computes `p=48`, `q=3/4`, then `(s+48)+3/4` in that order.
After iteration i, `s=195*i/4`, ending at **487.5**. Every intermediate is
a dyadic rational whose reduced numerator fits within 24 bits. Each SINGLE
store and each arithmetic result is therefore exact, including at x87's
lowest supported precision, independently of rounding mode. No reassociation,
overflow, underflow or division-by-zero assumption is needed. This proof does
not apply to FPCSEX's runtime inputs.

There are no calls or other observers inside the loop. After the first
checkpoint returns, the exact normal finite arithmetic cannot create another
floating exception. Consequently later checkpoints are redundant, but the
first one is not. All final numeric globals remain stored before the first
PRINT: this does not assume printing cannot observe them. For supported
event-free, non-resumable-error configurations, intermediate loop stores have
no observer. No relaxation of rounding, reassociation or runtime-entry DF/DS
is required. As throughout the optimizer, this is a language/FP-effect contract,
not preservation of debugger-visible instruction addresses or FPU bookkeeping.

PDS/VBDOS reference (binary32 values written as their exact bits):

```asm
mov dword [a],040000000h     ; 2
mov dword [b],040800000h     ; 4
mov dword [c],041000000h     ; 8
mov dword [s],0
mov word  [i],1
wait                       ; pending exception sees these five stores
mov dword [p],042400000h     ; 48
mov dword [q],03F400000h     ; 3/4
mov dword [s],043F3C000h     ; 487.5
mov word  [i],11
```

QB reference starts with `wait`, before any store, then writes only the final
seven globals: a=2, b=4, c=8, p=48, q=3/4, s=487.5 and i=11. Initial s=0
and i=1 have no observer between this checkpoint and their final assignments.
Both references finish with the same four runtime calls (verified in all
three objects' EXTDEF call sites):

```asm
push word descriptorS       ; 6
call far B$PSSD             ; 20
push dword 043F3C000h       ; 6: binary32 487.5, passed by value
call far B$PER4             ; 20: retain SINGLE formatting
push word descriptorDone    ; 6
call far B$PESD             ; 20
call far B$CENP             ; 20
```

The output subtotal is **3*(6+20)+20 = 98**. Each immediate store costs
2+4=6 and WAIT costs 5 in the common ranking model. Thus the complete targets
are **PDS/VBDOS: 9*6+5+98=157; QB: 7*6+5+98=145**. These are modeled costs,
not hardware timings. The report selects the reference from the object's
compiler COMENT record, never its filename. Unknown compiler identities and
event-enabled configurations remain provisional. FPCSEX still needs its own
reference; this constant-input proof does not apply to it.

## CMPORD — constant signed comparisons

For ordinary event-free configurations, the source defines four ordered
LONG pairs: (-1,1), (-2147483648,2147483647), (65535,65536), and
(0x12340000,0x1234ffff). In every pair the first value is strictly smaller.
For each pair, the six printed comparisons therefore have these two
results, in source order:

| Relation | Forward | Reverse |
|---|---:|---:|
| `<` | -1 | 0 |
| `<=` | -1 | 0 |
| `>` | 0 | -1 |
| `>=` | 0 | -1 |
| `=` | 0 | 0 |
| `<>` | -1 | -1 |

The eight initial LONG stores are unobservable: only the main body names
the variables and the module has no error or event handler. No numeric address escapes to printing,
so its subsequent calls do not invalidate these scalar constants. There
is no intervening assignment, unknown user call, floating operation or
event poll in this source/configuration. Signed comparison does not trap.

The independent listing consists of this sequence for each of the 24 rows, with the table's literal answers:

```asm
push word labelDescriptor
call far B$PSSD
push word forwardAnswer
call far B$PSI2
push word reverseAnswer
call far B$PEI2
; after all 24 rows
push word doneDescriptor
call far B$PESD
call far B$CENP
```

In the scoreboard model, each push costs
2+4=6; each retained output/termination call costs 20. Thus the complete
reference is **24*3*(6+20) + (6+20) + 20 = 1918** units. No B$CPI4
call, compare, conditional branch or intermediate Boolean store remains.
This derives the denominator from source states and required output—not
from the optimizer's measured total or a percentage of BC's cost.

The QB/PDS/VBDOS emitted listings independently match the 24 PSSD, 24 PSI2, 24 PEI2, one PESD and one CENP calls.
All three linked baseline/optimized executions match all 24 golden rows.
Event-enabled variants still need event-preserving references and remain
provisional. Registering this target changes no emitted instruction.

## CHAIN — nested integer division and remainder

Ordinary event-free builds have constant source inputs and no error handler.
Use signed division truncated toward zero, not Python's negative floor division:
`1073741831 = 27 * 39678839 + 2413178`, and
`2413178 = 24 * 100003 + 13106`. The inner XOR-derived divisor is 1 for
the first two remainders and 3 for the quotient rows. Negative remainders
retain the dividend's sign. No divisor is zero and no quotient overflows.

| Output row | r | a at the output call |
| --- | ---: | ---: |
| ONE | 0 | 1073741831 |
| CONST | 0 | 1073741831 |
| CONST2 | 13106 | 1073741831 |
| MODMOD | 13106 | 1073741831 |
| DIVDIV | 9 | 1073741831 |
| NEGMOD | -13106 | -1073741831 |
| NEGDIV | -9 | -1073741831 |

Retain all numeric assignments at their original positions relative to output
calls. A complete symbolic reference is the following straight-line expansion;
`row` is an assembly macro expanded seven times, not a runtime helper:

```asm
mov dword [a],1073741831
mov dword [b],39678839
mov dword [c],-1049330653
mov dword [d],100003

; row label,value expands to:
;   mov dword [r],value
;   push word label
;   call far B$PSSD
;   push dword value
;   call far B$PEI4
row ONE,0
row CONST,0
row CONST2,13106
row MODMOD,13106
row DIVDIV,9
mov dword [a],-1073741831
row NEGMOD,-13106
row NEGDIV,-9
push word DONE
call far B$PESD
call far B$CEND
```

Target: **482 modeled units** = twelve stores × 6 + fifteen
(immediate push + output call) pairs × 26 + termination 20. Each DWORD
argument can use a 386 operand-size prefix on all three compiler configurations.
The hand-derived seven answers match the checked-in golden file. This is a
source-level reference, not a claim that unknown machine calls may be ignored
by MIR analysis. Event variants still require their own reference.

Registering the target changes no emitted assembly. The checked-in CHAIN objects
contain only five result rows, while the current source has seven. Their costs
(QB/PDS 682, VBDOS 630) must not be divided by this denominator. The scoreboard
requires seven `B$PEI4` output sites or reports PROVISIONAL. Fresh builds of the
current source execute all seven rows correctly on QB, PDS and VBDOS. All 15
CHAIN fixtures have now been regenerated from that source with zero severe
compiler errors and updated manifest hashes. Three historical five-row objects
remain as `fixtures/regressions/chain5-*.obj` solely to test stale-reference
rejection; their original source is unavailable, so they are not runtime goldens.
Those fresh optimized objects cost **962 on QB/PDS (2.00×)** and **882 on
VBDOS (1.83×)**. This exposes a real above-target case hidden by the stale
five-row fixtures. The remaining constant arithmetic crosses output calls and
compiler-generated frame temporaries. A fresh PDS trace locates the first loss
at the label-argument push, **before** the print call: the write to `[sp-2]`
invalidates known `[bp-1a]`/adjacent frame bytes because stack/frame disjointness
has not been proved. The later load of the outer divisor consequently remains
unknown. The required work is a sound frontend stack/frame-region proof,
including unknown stack depth and escaped locals, not blanket call preservation.

```asm
; Current PDS, symbolic frame temporary shown
mov word [outerDivisor],1
mov word [outerDivisor+2],0
push labelONE                ; current alias facts forget outerDivisor here
call far B$PSSD
; ... first result output ...
mov ecx,[outerDivisor]        ; should still be known 1 if regions are disjoint
; ... inner remainder ...
idiv ecx                     ; remainder by 1 could then fold to 0
```

Refreshing fixtures and tracing facts change no optimizer instructions; this
listing is the measured remaining work, not an implemented before/after win.

### Main-frame geometry verified from the shipped runtimes

`runtime/inc/addr.inc` places `SZ_FRAME` at header offset 0x22. CHAIN records
24 local bytes on all three compilers. The fixed part below BP is **not**
uniform: the shipped `rtutil.asm` `B$FRAMESETUP` success path gives:

| Runtime | Establish BP | Fixed pushes after BP | Allocate locals | CHAIN entry SP relative to BP |
| --- | --- | ---: | --- | ---: |
| QB 4.5 | 0x41–0x42 | 5 words, 0x44–0x4b | `sub sp,cx` at 0x4c | -34 |
| PDS 7.1 | 0x9c–0x9d | 9 words, 0x9f–0xaa | `sub sp,cx` at 0xab | -42 |
| VBDOS | 0xb4–0xb5 | 10 words, 0xb7–0xc9 | `sub sp,cx` at 0xca | -44 |

QB reads the header through ES directly. PDS/VBDOS fetch it through their
module-address helper: CL=0x22 becomes a signed byte offset, added to the
module offset before `lodsw`. `_main` calls FRAMESETUP and then transfers to
header+0x30 by a balanced push-segment/push-offset/RETF sequence. The routine's
failure arms do not establish this successful-entry geometry. These are local
routine offsets, not linked executable addresses or a complete callee contract.

Library SHA-256 identities for this inspection:

```text
BCOM45.LIB   5b1c7a6fbb102e3e47acaa38349efa9bf1dae674e57f4d86d170e920086c8996
BCL71ENR.LIB 873fde67aa6fcf27961ec76d9f57ea8a621f6d16ea064da3312aa8d9e3a8c117
VBDCL10E.LIB 59ad49b055c4829528301e512abf9b8b0955181024c18282a49839e6c0680301
```

Frontend implementation must track subsequent SP/BP/SS changes and callee
cleanup, prove argument ranges disjoint from reserved locals, and separately
prove a call cannot reach a local through an escaped address. Entry geometry
alone does not justify preserving frame facts across arbitrary calls. Do not
change the global STACK/FRAME alias rule or reuse QB's 10-byte fixed layout
for the other two runtimes. No optimizer change or cost reduction is claimed
by this inspection.

### Implemented stack/frame exclusions

The raise now tracks entry-relative stack depth through instructions and
established callee cleanup. Conflicting CFG inputs, unbalanced loops, unknown
calls, interrupts and SP/BP/SS replacements lose the fact. A push/pop gets an
exclusion only when its whole range stays below the reserved local area.
Known OWN-effect calls can exclude those locals only when no frame/stack
address escapes in the body and their explicit register inputs do not include
SP/BP. Procedure entries, event-enabled modules and error-handler modules
remain unchanged. MIR passes consume byte-range exclusions, not registers or
runtime frame sizes.

Existing constant propagation then removes all six remaining divisions in
the current seven-row CHAIN program. Representative PDS CONST2 computation:

```asm
; before
mov ebx,[bp-22h]
mov ecx,100003
mov eax,ebx
cdq
idiv ecx
mov eax,edx
mov [r],eax
; after
mov dword [r],13106
```

The numeric assignments and output calls stay in source order. Modeled cost
falls **962 → 530** on QB/PDS and **882 → 530** on VBDOS, **1.10×** the 482
target on all three. PDS's object shrinks **1694 → 1524 bytes**. Compiler frame
stores still remain, so this is not claimed to reach the exact reference.
Fail-first tests retain the emitted-MIR division symptom; negative cases cover
pointer escapes and unknown/conflicting depth. Actual CHAIN (seven results),
PRESSX and FPDEEP (eleven results) executions pass on all three compilers.
The exclusion belongs only to a PUSH's implicit stack write or a POP's implicit
stack read. Their explicit memory operand retains its original alias facts:
`push [local]` reads that local, and `pop [local]` writes it. A follow-up
fail-first regression caught those explicit accesses incorrectly excluding
their own frame range. Fixing the metadata changes no instruction by itself;
it prevents later passes from treating the explicit access as disjoint.

## JUMPS — constant-trip dispatch and conditional branches

`FOR k=1 TO 3` executes three iterations, not the scorer's former fallback
of ten. Both inputs are constant (`a=305419896`, `b=252645135`). Expand the
three iterations and select each ON/CASE arm using that iteration's k:

| k | ON expression | ON result | CASE expression | CASE result |
| ---: | --- | ---: | --- | ---: |
| 1 | a AND b | 33818120 | a + b | 558065031 |
| 2 | a OR b | 524246911 | a - b | 52774761 |
| 3 | a XOR b | 490428791 | -a | -305419896 |

No arithmetic overflows. Retain a/b initialization, each source assignment to
r and k, k=4 after the loop, and every print call in source order. Compiler
SELECT temporaries have no source-level observer or escaping address.
This complete symbolic listing uses `row` only as an assembly macro:

```asm
mov dword [a],305419896
mov dword [b],252645135
; row label,index,value expands to:
;   mov dword [r],value
;   push word label
;   call far B$PSSD
;   push word index
;   call far B$PSI2
;   push word equalsDescriptor
;   call far B$PSSD
;   push dword value
;   call far B$PEI4
mov word [k],1
row ON,1,33818120
row CASE,1,558065031
mov word [k],2
row ON,2,524246911
row CASE,2,52774761
mov word [k],3
row ON,3,490428791
row CASE,3,-305419896
mov word [k],4
push word DONE
call far B$PESD
call far B$CENP
```

Target: **742 modeled units** = twelve stores × 6 + six rows × four
(push + call) pairs × 26 + DONE's pair 26 + termination 20. This derives
from the source, not a scaling of current output. Event variants remain
provisional. The model sums instructions in alternative branch blocks; it
is a static ranking, not a measured execution time or path-frequency profile.

Correcting the trip count changes **PDS 4070 → 1270 modeled units**, with no
emitted assembly change. Current QB/PDS/VBDOS costs are **1288/1270/1204**,
or **1.74×/1.71×/1.62×**. All remain above the goal after fixing the instrument.

The pass dumps identify two representation limits: B$OGTA remains a CALL
with multiple successors rather than a semantic multiway branch, and the
unroller accepts only a linear floating loop body, not this branched integer
body. Its existing OWN memory contract already excludes unescaped module
numeric globals; adding another generic call-preservation rule is not the
missing optimization. Next expose dispatch semantics in the raise and enable
bounded CFG unrolling, allowing SCCP to resolve each cloned iteration's arms.

### Dispatch recognition contract

The shipped runtimes do **not** implement an unrestricted switch default.
Manual disassembly of `gosub.asm` in all three libraries establishes:

| Selector (unsigned INTEGER) | Control |
| --- | --- |
| 1 through the table count | Corresponding entry, in table order |
| 0, or greater than count but at most 255 | Byte immediately after the table |
| 256 through 65535 (including negative signed INTEGERs) | `B$FrameFC` |

The rejecting test is `or bh,bh / jne`: QB at 0054/0056, PDS at
005B/005D, VBDOS at 005F/0061. Its target is a relocated near jump to
`B$FrameFC`, independently resolved through FIXUPP/EXTDEF at QB 008F,
PDS 009B and VBDOS 009F. The default returns are QB 008B, PDS 0097 and
VBDOS 009B. Decode these branch roots separately: PDS/VBDOS's linear
listing overlaps their `push es / push dx` with a preceding `cmp` encoding.

Recognition must therefore prove the selector is in 0..255 or preserve the
exceptional behavior explicitly; mapping every non-case value to the default
is wrong even when ordinary JUMPS output passes. Keep ordered cases separate
from the deduplicated successor set, since repeated destinations are legal.
Replacing the call also needs a proof that its outgoing clobbered values are
unobserved, or explicit semantic replacements for observed results. The existing
OWN memory contract alone proves neither fact. JUMPS's three iterations satisfy
the selector range, but general dispatch recognition cannot assume that range.
This contract audit changes no emitted assembly.

The raised PDS JUMPS body was checked directly on 2026-09-11. The selector
is the single two-byte argument at `0052`. None of the call's seven defined
values has an instruction reader. One reaches the SI phi at `0151`; that
phi has no instruction or phi users. Prune dead phis before testing whether
the call's results are observable, rather than weakening its clobber contract.

Implementation order: represent ordered cases, normal default and invalid
selector behavior in MIR; teach SSA/CFG cloning and SCCP that representation;
then lower surviving switches to branches and repair successor phis. LLVM's
`llvm/lib/Transforms/Utils/LowerSwitch.cpp` at
`338e0c94943a6fb917c276bbbd9ff4b6cd6dd71e` is the local reference for the
last step, particularly `FixPhis` and `NewLeafBlock`. Its unrestricted default
does not replace BASIC's invalid-selector path. Runtime-specific recognition
stays in raise; no optimizer may inspect the helper name or selector register.

## FPDEEP — exact constant floating expressions

**Current PDS target: 1317.** The former 1086 numerical/printing listing below
omitted synchronization and numeric stores. The complete reference retains
those observations and has been assembled with JWasm, linked and executed.
QB/VBDOS, event-enabled and resumable-error builds remain provisional; their
checkpoint placement needs separate verification. This target correction
does not change llrm's emitted code.

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

Numerical reference: **9×104 + 2×52 + 46 = 1086**. This is the same static
ranking used by the scoreboard, not elapsed time or a hardware-cycle claim.
The optimized program has not reached this form. Constant propagation computes
the indexed SINGLE answers, but the DOUBLE tail still contains floating
arithmetic. Removing its initialization barrier and the remaining synchronization
and stores needs separate proofs.
Event-enabled builds remain provisional because their event observations
cannot be discarded by this reference.

### Current DOUBLE-tail blocker (2026-09-10, compiler 70ee58b)

Historical optimization analysis; the later complete PDS reference supersedes
the denominator's provisional status, not the copy-recognition limitation.

Fresh per-pass dumps of QB `/O` and PDS `/G2` distinguish the source-level
constant from what the raise actually knows. PDS initializes `d=12` using four
`MOVSW` instructions at original offsets 0x154–0x157. All four remain OPAQUE
in the initial MIR; its subsequent binary64 load at 0x158 has no constant fact.
This is not a missed arithmetic identity in CSE. `src/frontends/bc/raising_copies.rs`
already scalarizes explicit-direction, proven-selector copies, but its dataflow
forgets traversal direction across calls. No local CLD establishes it here.

Current PDS emitted tail (relocations named; setup and printing omitted):

```asm
; d = 12 is copied from the literal pool
movsw
movsw
movsw
movsw
fld qword [d]
fld qword [d]
fadd qword [d]
fdivp
fmul qword [d]
fstp qword [e]
wait
```

The independent numerical destination is `d=12`, `e=6`, DSQ=144 and
DRATIO=6. An observation-preserving listing must place d/e stores and checks
relative to the retained output calls, not merely push those two answers.
Neither the target nor emitted code was changed by this audit. PDS's current
object is 1838 bytes; the modeled cost remains 1777 (QB 1570).

Next: verify the returning runtime's direction/selector contract from its
implementation, represent that fact at the ABI/raise boundary, and let the
existing copy scalarizer expose ordinary scalar memory operations. Do not
assume DF=0 at entry or after every call, and do not teach an MIR pass about
MOVSW. General copy promotion remains incomplete until unknown contracts and
overlap retain their original behavior. The 1086 denominator stays provisional.

### Exact DOUBLE-store follow-up

The print routine's source reaches device-dependent output/flush vectors, and
its numeric-conversion header contradicts other register-clobber evidence.
It does not establish a universal DF/DS return fact. Copy recognition stays
conservative; no new runtime guarantee was invented.

QB already raises d=12 and e=6 as exact facts. Its separate blocker was that
`floatfold.stored` accepted only binary32. It now accepts exact binary64 too,
retaining the pending-exception check and one whole-width semantic STORE.
Lowering alone splits that store into two immediate dword stores. Inexact
conversion still uses floating instructions. QB's actual tail changes from:

```asm
fld qword [literal12]
fst qword [d]
fld st0
fadd st0
fld st1
fxch
fdivp
fmulp
fstp qword [e]
wait
```

to:

```asm
wait
mov dword [d],0
mov dword [d+4],040280000h   ; binary64 12
mov dword [e],0
mov dword [e+4],040180000h   ; binary64 6
```

Printing and the checks after printing calls remain. QB FPDEEP now has no
floating arithmetic; modeled cost falls **1570 → 1317**, while its fixture
object grows **1727 → 1742 bytes**. All eleven runtime answers pass on QB,
PDS and VBDOS. PDS/VBDOS retain their opaque copy and are not claimed improved.
This does not validate the old numerical-only denominator.

### Complete PDS reference — 2026-09-11

`tools/references/fpdeep.asm` is hand-written JWasm, not llrm output. The
wrapper preserves the hash-pinned BC object's data and runtime relocations;
it uses neither MIR optimization nor llrm instruction selection.

Keep p(1..3)=12,28,60 and k=4, and store i=1,2,3,4 at the source iteration
boundaries. For each of nine indexed rows, check pending exceptions before
the exact arithmetic, store q's SINGLE bits, retain the three label/index/
equals output calls, then check again before the exact CLNG result is printed.
Calls can leave pending exceptions: those second checks are not redundant.

The DOUBLE tail stores d=12 before its first check, then e=6, retaining two
dword stores per DOUBLE. Keep a check after each DSQ/DRATIO label call before
printing 144/6. The source's END remains B$CEND. All arithmetic/conversions
are exact normal finite values under every supported precision/rounding mode,
as derived in the table above. This does not authorize deleting runtime-input
FPCSEX operations or inferring a DF guarantee for PDS's MOVSW initializer.

| Required work | Count | Ranking cost |
| --- | ---: | ---: |
| Initial p/k stores | 4 | 24 |
| Loop-counter stores | 4 | 24 |
| q stores before output | 9 | 54 |
| d/e dword stores | 4 | 24 |
| Pending-exception checks | 21 | 105 |
| Original output calls/arguments and END | full listing | 1086 |
| **Total** | | **1317** |

Representative difference (reference, not a new optimizer pass):

```asm
; BC's first row
fld dword [p1]
fmul dword [p1]
fstp dword [q]
wait
; three output calls
fld dword [q]
call far B$FIST
push dx
push ax
call far B$PEI4

; independent reference
wait
mov dword [q],043100000h       ; 144.0, exact SINGLE
; the same three output calls
wait
push dword 144
call far B$PEI4
```

Native-FPU run evidence: `/tmp/qbopt-fpdeep-jwasm.xq6rYz`. BASE, REF and OPT
link without errors and print byte-identical eleven answers plus DONE; all
return to DOS. `result.png` was captured and inspected. FPS is inapplicable
to this console program. Raw scoring independently gives REF=1317 and the
unchanged OPT=1771: **1.34x**, not a measured runtime speedup.

Regression checks count the assembled stores/checkpoints, validate q/counter/
DOUBLE values and all 42 runtime calls, and reject unaudited input/absent
relocation metadata. Removing the nine arithmetic-entry checks makes the
test fail (12 WAITs instead of 21); the mutation is restored. The scorer
selects the accepted scope from object compiler/switch metadata, not filenames.

## NOTS and NEGNOT — constant expressions across output statements

These ordinary, unchecked, event-free programs initialize two unescaped
LONG scalars, then print expressions over them. Their sources contain no
READ, user callback, assignment to either input after initialization, or
runtime-dependent arithmetic. A source-level compiler can evaluate every
expression modulo 32 bits. This reference does not grant llrm permission
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

### Complete event-free SUBEXP reference

The arithmetic comparison above is not the whole-program denominator.
The fixed source values are x=11, y=5, p=32, q=48, all representable as
INTEGER without overflow. In these fixtures x/y and p/q occupy consecutive
word locations. A good backend can encode each adjacent pair as one dword
store; this is a backend storage-layout choice, not machine-specific MIR
arithmetic. All four numeric globals are established before the first output
call, just as in the source. No store is moved across a call.

The established print contracts write runtime/string-owned memory, not these
numeric cells, and do not enter user code in this event-free program. The
numeric arguments therefore remain constants across the descriptor-print
calls. All calls, including their possible I/O errors, remain in source order.

```asm
mov dword [x],0005000Bh       ; x=11, y=5: 6
mov dword [p],00300020h       ; p=32, q=48: 6
push word labelP             ; 6
call far B$PSSD               ; 20
push word 32                 ; 6
call far B$PEI2               ; 20
push word labelQ             ; 6
call far B$PSSD               ; 20
push word 48                 ; 6
call far B$PEI2               ; 20
push word labelDone          ; 6
call far B$PESD               ; 20
call far B$CENP               ; 20
```

Two stores, five push/call pairs and termination give
**2*6 + 5*(6+20) + 20 = 162** independently of BC's measured cost. This
confirms the existing target without ratio-based rescaling. At `473262d`,
PDS /G2, QB /O and VBDOS /G3 each cost 168 (1.04x): their output combines
x/y but still stores p/q separately. No generated code or denominator is
changed by this accounting. Event-enabled variants remain provisional.

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

Two dimensions and dynamic. The first subscript varies fastest in storage:
the inner `c` counter has byte stride 42 (21 elements of two bytes), and
the outer `r` counter has byte stride two. Thus one `add` per pass is what it costs once an induction
variable carries it. BC recomputes the whole thing from the subscripts:

```
0058  add ax,[c]          | ; outside both loops: es, and di = array base
005c  mov bx,ax           | ; outer: bx = &m(r,1), ax = r+1, cx = 10
005e  mov ax,[w]          | inner:
0061  imul word [c]       |   mov  [es:bx],ax
0065  add ax,[r]          |   add  dx,ax
0069  shl ax,1            |   inc  ax
006b  mov dx,bx           |   add  bx,42
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

Current check at `aa6194c`: this gap is closed for the ordinary builds.
PDS and VBDOS score 2086/1834 (1.14x), QB scores 2114/1834 (1.15x).
The PDS inner loop is `mov [es:di],si; add cx,si; add si,1;
add di,42; cmp si,dx; jne inner`. The segment load and initial address are
outside both loops; the outer loop increments its address by two.
These are static costs, not elapsed times. Correcting the historical
listing's swapped subscript labels and stride does not change the target's
instruction cost or its denominator.

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

The investigation below is historical. Literal FPCSE now has the complete
[entry-observation and final-store reference](#fpcse--complete-reference-with-entry-observation-and-final-stores)
above; FPCSEX's runtime-input reference remains provisional. Current measured
costs belong in [the target snapshot](target-coverage-current.md), not the
dated intermediate totals below.

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
see `tools/references/readme.md`. The old reassociated, unrounded target is
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

## DIVMOD — resumable errors with constant integer arithmetic

The current source prints twenty numeric results, then `DONE`; older fixture
objects printed only eleven numeric rows and are not comparable.  The optimal
normal path folds every proven nonzero constant divide, remainder and multiply.
It still installs the ON ERROR handler, retains `caught = 0` because a printing
error can resume into an observation of it, and retains the handler itself.

Target: **1174 modeled units** = 32 for error registration, 1040 for twenty
label/value output pairs, 10 for the observable `caught` state, 46 for DONE and
termination, and 46 for ERR/store/RESUME.  The handler's mod/ref summary is part
of the proof: error-capable calls may write `caught` and escaped string state,
but do not thereby write unrelated numeric globals.

## FPEMU — exact finite floating results

Eleven label/value rows contain exact binary-power arithmetic.  The FCMP row
prints one label and two booleans, followed by DONE.  `sqrt(1048576)` is the
exact rational square 1024; non-squares, negative values and inexact results are
not licensed by this reference.  Pending exception checks remain wherever an
operation is not proven exact and exception-free.

Target: **696 modeled units** = 572 for eleven ordinary rows, 78 for FCMP's
three outputs, and 46 for DONE and termination.

## PROCS — public BASIC procedure bodies remain link-visible

Whole-module facts may specialize the three calls in main, but they may not
delete the public `TWICE` and `REPORT` implementations.  String assignment and
temporary disposal retain their allocation-failure behavior, as do the numeric
by-reference temporaries at each call site.

Target: **533 modeled units** = 334 for main, 63 for the generic `TWICE` body,
and 136 for the generic `REPORT` body.

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

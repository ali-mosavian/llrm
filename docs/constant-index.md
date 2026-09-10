# Complete shift facts and constant array offsets

Constant analysis previously assigned a shift the minimum width of its
operands. A byte-sized count consequently narrowed a word-sized result to a
byte fact. The result now retains the value's proven width. Indexed memory
facts also resolve a known offset through the existing no-wrap address proof;
unknown offsets, incomplete values and unresolved segments remain unknown.
Neither change selects a CPU or instruction sequence.

SUBEXP's PDS emitted code changes from:

```asm
mov ax,10h
shl ax,1
mov [p],ax
; print label
push word [p]
```

to:

```asm
mov ax,20h
mov [p],ax
; print label
push 20h
```

Code shrinks by four bytes; the object including relocation records shrinks
from 788 to 779 bytes. Both printed answers pass on QB, PDS and VBDOS (six
checks). SUBEXP is the only changed emission among the ordinary `*-p-g2.obj`
fixtures. This is not a runtime timing result.

Experimental FPDEEP expansion now exposes 39 exact floating values rather
than zero: squares 144/784/3600 and ratios 6/14/30 are proven independently
for each iteration. Expansion remains disabled in the normal pipeline; this
does not claim those floating computations have been removed or accelerated.

Five regression cases fail with the previous analysis, including the actual
FPDEEP fixture. The focused constant, floating-fact and expansion tests pass
(59 tests). Before/after dumps of every stage and SUBEXP runtime artifacts:

`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-constant-index-wcdqjg68`

## Exact conversion results reach arguments

The next step exposes proven integer conversion results to argument
propagation only. It does not authorize deletion of floating effects.
In expanded FPDEEP, the six argument values change from converted SSA
temporaries to constants, in order: `144, 6, 784, 14, 3600, 30`.
The floating operation list remains identical, including each conversion.
Fractional and out-of-range results remain unknown; negative exact integers
use the result type's bit pattern.

The production fixture regression fails with the previous folding function.
49 focused tests pass. Ordinary PDS fixture emission is unchanged.
An isolated experimental run enabling expansion and ordered emission passes
all 11 FPDEEP output checks on PDS. These experimental switches are still
off in the production pipeline. Artifacts:

`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-fp-arguments-vss0qb5s`

This is not yet the target-sized program: arithmetic and conversion work
remain. The missing mix facts were traced to the memory operand at MIR
address `0x12c`, not to the division: its left input is proven exactly
1/2, 3/4, 7/8, but the constant-pool operand is unknown at that point.
Do not replace this missing memory proof with blanket pool immutability.

### Where the pool fact disappears

At the current PDS MIR boundary, the entry fact for segment 9 offset 0x1e
is `0x44800000` (SINGLE 1024). It is still present before the expanded
latch's first operations. It disappears across the `B$PSSD` call at 0x81,
before the argument at 0x86, not across a floating instruction.

That call has an OWN-memory runtime contract, but its `beyond` summary
identifies program-data segment 5 only. `_out_of_reach` deliberately cannot
prove anything about segment 9. The escaped addresses include segment-9
string descriptors at 0, 6, 12, 22, 42, 50 and 62. They are pointer origins,
not proven byte extents. The literal occupies bytes 30 through 33.

The runtime printing contract permits string compaction to update live
descriptors, so this is not a justification for making BC_CN immutable or
pretending PRINT writes no memory. Preserving the literal across this call
requires bounded reachable-object effects established by the raise/runtime
contract, rather than interpreting absent escaped origins as disjoint bytes.
The six independently proven square/ratio conversions do not depend on
solving that additional memory-summary problem.

### Escape-source correction and descriptor layouts

Raw FPDEEP objects expose a second prerequisite for that proof. QB uses
`mov ax, OFFSET descriptor; push ax`, not PDS's immediate PUSH. The old
escape scan returned an empty set for QB despite seven string arguments.
It now recognizes adjacent immediate-register materialization and PUSH of
that same register. Known by-value numeric PRINT arguments remain excluded;
local copy setup addresses are not treated as passed arguments. This is not
a general inter-block pointer escape analysis or a byte-extent guarantee.

The QB descriptor origins are 16, 22, 28, 38, 58, 66 and 78 in segment 9.
PDS uses four-byte near descriptors at 0, 6, 12, 22, 42, 50 and 62, with
relocations from each descriptor's second word to its text. VBDOS instead
uses segment-9 offset/selector references into segment 16. A single assumed
four-byte descriptor layout across all three compilers would be wrong.
The QB fixture regression failed before the scan fix; 23 focused escape
and constant-call checks pass. All three primary FPDEEP emitted objects
remain byte-identical to before this scan fix: no speedup is claimed.

## Do not materialize an unread conversion result

Floating allocation now creates the integer reload only when the result
has a reader. This is allocation of a result, not a new LIR optimization
pass: the conversion and its synchronization remain unchanged.

```asm
; Before                         ; After, if the result is unused
wait                             wait
fistp dword [ownedTemporary]      fistp dword [ownedTemporary]
wait                             wait
mov eax,[ownedTemporary]
```

The before/after sequence is covered for both word and dword conversions.
Pinned results, phi inputs, explicit uses and operands retain the reload;
any opaque instruction or barrier conservatively retains all such reloads.
41 focused allocation tests pass, including two fail-first regressions.
Ordinary PDS fixture bytes and emission outcomes are unchanged. This does
not yet claim a suite speedup: unread results must survive the upstream
pipeline in a body whose readers are fully modeled to benefit.

## Exact load/conversion pairs become floating checks

`floatfold.discarded` now replaces an adjacent, exact load/conversion pair
with two `FCHECK` operations when neither its integer result nor its floating
intermediate has another reader. A live result, shared input, memory store,
barrier or absent exact proof keeps the pair. `FCHECK` observes pending
floating exceptions without computing a value; lowering selects `wait`.
The existing backend combines adjacent redundant waits.

This follows the distinction in local LLVM
`llvm/lib/Analysis/ConstantFolding.cpp:mayFoldConstrained`: exact evaluation
with `opOK` permits folding; a strict operation that changes exception status
must stay. qbopt additionally retains the observation point explicitly.
The MIR transformation does not name a register or machine instruction.

Actual PDS emitted before/after at the first square print:

```asm
; Before                         ; After
fld dword [scratch]              wait
fistp dword [bp-4]               push 90h
wait                            call B$PEI4
mov eax,[bp-4]
push 90h
call B$PEI4
```

All six exact pairs disappear across the expanded iterations. Temporary
stack space decreases from 48 to 24 bytes; PDS's object shrinks from 2176
to 2078 bytes. Static cost decreases from 4047 to 3663 (QB 4209 to 3861,
VBDOS 4045 to 3661). Both sides are expanded, so neither uses the erroneous
ten-trip estimate. These are cost estimates, not measured execution times.

All 33 FPDEEP output checks pass across the three compilers with experimental
expansion selected and no checker/emitter bypass. The fixture regression
fails with pair deletion disabled; focused guards cover shared/live values,
memory effects and retained checks. Stage dumps and execution artifacts:

`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-exact-checkpoints-k47dqj3q`

## Exact SINGLE stores and unused arithmetic

Exact SINGLE stores now write their proven bit patterns directly, retaining
a floating check. Their unused, exact floating producer chains similarly
become checks, which the backend can combine when adjacent. Inexact stores
remain floating conversions; neither rounding nor exceptions are ignored.

Actual first-square code in expanded PDS FPDEEP:

```asm
; Before                         ; After
fld dword [p+si]                 wait
fmul st0                         mov dword [scratch],43100000h
fstp dword [scratch]             wait
wait
```

`43100000h` is SINGLE 144. The nine stores across three iterations become
144/6/0.5, 784/14/0.75, and 3600/30/0.875. Floating operations fall from
71 to 20. PDS's object shrinks from 2078 to 1927 bytes. Static costs:

| Compiler | Before | After | After / target 1086 |
|---|---:|---:|---:|
| PDS | 3663 | 2166 | 1.99x |
| QB | 3861 | 2201 | 2.03x |
| VBDOS | 3661 | 2162 | 1.99x |

All 33 FPDEEP output checks pass with experimental expansion selected.
The default FPCSE path also benefits, passing output checks on all three
compilers: object sizes are PDS 922→878, QB 953→889, VBDOS 1098→1054.
No elapsed-time speedup is claimed.

QB FPCSE exposed an entry-accounting bug. The diagnostic claimed five bytes
overlapped, but the raw ranges showed no overlap: removing the first load
left source address 0x35 as the first survivor, owning bytes from entry 0x30.
Layout and object writing now use the explicit procedure entry rather than
the minimum surviving source address. A separate ownership regression ensures
inserted occurrences retain relocation identity without inheriting source
byte coverage. Both the actual QB emission refusal and chain-elimination
regression were observed failing before their fixes.

The targeted layout/relocation run had 4416 passes and 28 failures. All 28
also fail with the previous folding/layout/assembly functions restored in
process; they are not treated as passing or as newly introduced failures.
Stage dumps and runtime artifacts:

`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-exact-stores-s9k235ua`

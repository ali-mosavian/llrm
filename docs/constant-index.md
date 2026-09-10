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

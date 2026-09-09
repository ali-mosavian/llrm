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

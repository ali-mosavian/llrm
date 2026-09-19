# QB frontend compatibility ladder

The frontend advances through small executable programs before any qrender or
qb-quake substitution. A rung is complete only when the same source is accepted
by the selected Microsoft compiler, its raw object has been inspected, the new
frontend's source/HIR/MIR/LIR/emitted assembly stages are saved, and the fresh
object links to that compiler's runtime and produces the expected DOS output.
The runtime result is the primary fact; a compiler `/A` listing is not raw code
because VBDOS leaves back-patched frame operands displayed as zero.

Each run of `tools/qbstages.py` writes:

1. `00-input.bas`, the exact source;
2. `01-hir.json`, resolved typed HIR;
3. per-function semantic, optimized, and physical MIR;
4. per-function Intel/MASM LIR after each existing machine phase;
5. per-function inline-x87 LIR; and
6. `99-emitted-asm.asm`, including the QB runtime ABI envelope and the exact
   `retf n` cleanup encoded in OMF.

The last file exists because the ABI envelope is intentionally frontend-owned;
stopping at ordinary LIR hides `B$ENRA`, `B$EXSA`, module initialization, and
callee stack cleanup.

## FreeBASIC corpus as a source of cases

FreeBASIC's local checkout at `~/work/personal/fbc/tests/qb` supplies only BASIC
test scenarios and source fragments. Nothing uses FreeBASIC's ABI, runtime,
lowering, code shape, object format, harness result, or implementation. Cases
are adapted to legal QB/PDS/VBDOS source and printable output. FreeBASIC-only
preprocessor assertions and extensions are removed. Every adapted case is then
compiled with Microsoft BC; BC's object plus the selected Microsoft runtime are
the ABI evidence.

Initial sources and what they contribute:

| FreeBASIC test | Frontend rung |
|---|---|
| `local-suffixvar-overrides-shared-suffixvar.bas` | automatic locals begin zero and do not alias module storage |
| `call.bas` | zero/multiple arguments, SINGLE, STRING and array descriptor passing |
| `qbtypes.bas`, `literal_sizes.bas` | 16-bit INTEGER, 32-bit LONG, SINGLE/DOUBLE widths and literal typing |
| `align_qb.bas` | packed QB user-defined-type layout |
| `str.bas` | numeric-to-string temporaries and cleanup |
| `rnd.bas` | Microsoft-compatible random state and floating arguments |
| `redim-staticlocals-*.bas`, `static-and-option-dynamic.bas` | dynamic descriptor lifetime and STATIC procedure state |

The much larger general FreeBASIC suite remains useful later, but modern
objects, constructors, namespaces, threads and 64-bit types are outside this
frontend's stated language/runtime target.

## Runtime ABI rungs

### 1. Numeric frame zeroing

`frontends/qb/fixtures/runtime-frame-basic.bas` has one INTEGER and one LONG
automatic local. VBDOS BC's raw object and the fresh object both set `CX=6`,
`BX=0`, and call `B$ENRA`. The runtime body executes `sub sp,cx`, selects DS for
the zeroing destination, shifts the byte count by one and executes `rep stosw`.
Both objects read the locals below the 20-byte VBDOS runtime header and finish
through `B$EXSA`. The standalone fresh executable prints:

```text
FRAME ZERO
```

There is no additional stack-check call at this site in BC's raw object.
`B$ENRA` reserves and touches the requested extent; no separate limit compare is
visible on its returning path. Do not strengthen that observation into a claim
about every compiler mode until QB 4.5 and PDS have the same raw-object probe.

### 2. Large live automatic extent

`frontends/qb/fixtures/runtime-frame-stack.bas` keeps a 4096-byte fixed STRING
live by assigning it. BC and the fresh frontend both put `1000h` in `CX`, zero
`BX`, and call `B$ENRA`; the fresh local begins at `BP-1014h`, immediately below
VBDOS's 20-byte runtime header. Linked only with `VBDCL10E.LIB`, it prints:

```text
STACK RESERVED
```

and returns through `B$EXSA`.

### 3. Procedure ABI

Grow the probe through the argument shapes from FreeBASIC `tests/qb/call.bas`,
one at a time. BASIC calls push left-to-right, parameters remain at positive BP
offsets after frame insertion, and the callee emits `retf n`. The current
two-BYREF frame probe encodes `retf 4`; the stage renderer has a regression that
must show the immediate rather than the older misleading bare `retf` spelling.

`runtime-call-basic.bas`, reduced from the source scenario in FreeBASIC
`tests/qb/call.bas`, makes argument order observable with `50 - 8`. BC and the
fresh frontend push `50` then `8`. Since Pascal pushes left-to-right, the callee
reads the first formal from the higher address and the second from the lower:

```asm
pushd 50
pushd 8
call far ptr SUBTRACTPAIR
...
mov eax, dword ptr [bp+12]
mov ebx, dword ptr [bp+8]
sub eax, ebx
retf 8
```

The standalone fresh executable prints `CALL OK`. This rung found two general
frontend defects. Replacing the module's final `RETURN` with `B$CEND` cleared
the successors of every block, so both outcomes of an IF fell into its true
block. After that was fixed, parameters were still read in ascending declaration
order, computing `8 - 50`. The module-exit rewrite now preserves all non-exit
CFG edges, and only the QB ABI adapter reverses Pascal physical parameter
offsets; HIR parameter and argument order remain source order.

### 4. Bare zero-argument FUNCTION and LONG return

Microsoft BASIC permits a declared zero-argument FUNCTION to be referenced
without parentheses. QB 4.5, PDS 7.1, and VBDOS listings for
`bare-function.bas` all contain `call ANSWER&`, and both their executables and
fresh frontend executables print `42`. Name resolution checks an existing
variable first—so a FUNCTION body still reads and assigns its result cell—then
resolves a matching zero-argument signature before implicit-variable creation.

The public LONG ABI is `DX:AX`, including under VBDOS `/G3`; `/G3` changes
argument pushes, not function results. `external-long.bas` calls mgl's
`memAvail&`, whose raw library body ends with its high word in DX and low word
in AX. BC's listing stores AX and DX separately. HIR and optimized MIR retain
one whole LONG; frontend ABI physicalization receives two word results and
reconstructs that value after the call. A LONG return performs the inverse
split only at the ABI boundary. The Microsoft and fresh executables both print `-1` for
`memAvail& > 65535`.

### 5. Static STRING literal profile

`readonly-data.bas` prints one literal. Raw QB 4.5 and PDS 7.1 objects keep a
four-byte near string descriptor and its payload together in `BC_CN`: the
length word, a relocated near offset, then the character bytes. VBDOS instead
keeps the far descriptor and bytes in private `FSL_CONST`; `BC_CN` contains a
near bridge to it and to a shared selector word. These are runtime data-layout
profiles, not parser-dialect differences.

Using the VBDOS form with QB or PDS linked successfully but printed garbage
(`63h` under QB and `E9h 09h` under PDS) instead of `A`. Exact-object
regressions now distinguish the layouts, and the Microsoft and fresh
executables for all three profiles print:

```text
A
```

The adjacent dumps are in `build/qbstages/static-string-{qb45,pds71,vbdos}-round`.
Their HIR data differs by runtime profile, while semantic MIR remains one
descriptor-address argument to `B$PESD` and LIR remains Intel/MASM.

### 6. Runtime clock

`timer-basic.bas` reads `TIMER`, waits for it to change, and prints whether a
later read advanced. Raw QB 4.5, PDS 7.1, and VBDOS objects call `B$TIMR` with
no stack arguments. Each runtime stores a SINGLE in its own cell and returns a
near pointer in AX; the caller must load through that pointer. The fresh HIR
therefore contains an effectful call followed by an explicit SINGLE load, and
all three fresh executables print `-1`.

This rung found `SYS_TIME_INIT` treating `TIMER` as a zeroed implicit local,
which made its first calibration loop infinite. The PDS library had also been
audited but omitted from the profile-keyed contract table; its measured normal
return is a bare `retf` with zero cleanup.

### 7. Floating FUNCTION result

`float-function.bas` calls a one-argument `FUNCTION ... AS SINGLE` and prints
`2`. Raw VBDOS `SYS_TICK_HZ` and the isolated compiler probe establish the
Microsoft ABI: the caller pushes a near destination after ordinary formals;
the callee stores four bytes through `[bp+6]` (adjusted only by the native
shell), returns that pointer in AX, and includes the hidden word in `retf n`.
The isolated fresh executable prints `2` under QB 4.5, PDS 7.1, and VBDOS.

HIR retains the semantic float result and explicit hidden ABI operand. Late
QB physicalization changes the call result to the AX pointer plus an `fload`,
and changes the callee return to `fstore [pointer]`, AX return, and exact stack
cleanup. No MIR kind or backend convention was added.

### 8. One managed STRING temporary

`runtime-string-one.bas` uses `COMMAND$`, then assignment to one local dynamic
STRING. It emits `CX=6`, `BX=1`, one `B$FCMD`, and clean return through
`B$EXSA`. With command tail `HELLO WORLD`, the standalone executable prints
`HELLO WORLD`.

### 9. Nested managed STRING temporaries

`managed-temporaries.bas` uses `RTRIM$(LTRIM$(COMMAND$))`. Raw VBDOS output
emits `BX=1`: only the frame-owned `commandLine` descriptor belongs to
`B$ENRA`; runtime-produced descriptors remain on the runtime temporary chain.
Its standalone output is `HELLO WORLD`.

### 10. Local STRING array

`string-array-element.bas` allocates, fills, reads, and leaves a fixed-bound
dynamic STRING array. Its descriptor is at `BP-26h`, its data offset is loaded
from descriptor `+0Ah`, and it calls `B$ERS1` before `B$EXSA`. The standalone
executable prints `HELLO` padded to the 64-byte fixed-string destination and
returns normally.

### 11. STRING FUNCTION result lifetime

`string-function-result.bas` returns `items(2)` from one `FUNCTION AS STRING`,
assigns the result to a local STRING in a second function, and compares it with
`"yes"`. The standalone fresh executable prints `-1`.

Raw VBDOS `COM_ARG` establishes the boundary: the function owns a four-byte
result descriptor in its `B$ENRA` frame, assigns the array element to it with
`B$SASS`, passes that descriptor to `B$SCPF`, and returns the near descriptor
pointer left in AX. `B$SCPF` copies the value to the runtime temporary chain so
it survives the immediately following `B$EXSA`. Returning the descriptor's
four bytes instead invents an AX:DX ABI and leaves the caller without a valid
descriptor pointer.

The saved round is `build/qbstages/string-function-result-round`. Its stages
make the boundary visible without changing MIR or the backend:

```text
input:  value = secondItem(items())
HIR:    FUNCTION SECONDITEM returns near*string; final call is B$SCPF
MIR:    v10 <- call B$SCPF(v9)
        return v10
LIR:    push ax
        call far ptr B$SCPF
MASM:   call far ptr B$SCPF
        call far ptr B$EXSA
        retf 2
```

This was reduced from QRender's apparent freeze. The DOSBox debugger stopped
inside `B$SLEP`, which is `SYS_ERROR` deliberately waiting for a key after
printing `Expected yes/no at line # 35`. The bad value came through
`COM_YESNO -> COM_ARG`; it was not a tokenizer loop. After the fix, a fresh
COMMON substituted into the legacy build renders five frames, as does the
fresh MAIN+COMMON substitution. Later rounds supersede the old all-source
stopping-point observation.

### 12. Non-addressable BYREF STRING argument lifetime

`asc-literal.bas` passes `"m"` to `FIRSTCODE(text AS STRING)`. The callee first
calls `LEN(text)` and then `ASC(text)`. Passing the immutable literal descriptor
directly appeared to work in an `ASC`-only probe, but `B$FLEN` consumed that
runtime temporary and `B$FASC` then raised BASIC error 5. The reduced fresh
executable now prints `109`.

Raw VBDOS `D_SURF.OBJ` shows the general source ABI rule at `LS_INIT
012f..013f`: BC assigns the expression with `B$SASS` into an owned dynamic
STRING descriptor and passes that descriptor to `LS_LCHAR`. The same pattern
appears for the `MID$` result in `LS_ANIMATE 02a2..02c8`. A genuine dynamic
STRING lvalue still passes its descriptor directly and retains BYREF aliasing;
only literals, fixed strings, concatenations, and function/runtime results are
materialized.

The saved round is `build/qbstages/asc-byref-owned-round`:

```text
input:  code = firstCode("m")
HIR:    address literal; address $stringArg4; call B$SASS; call FIRSTCODE
MIR:    call B$SASS(v1, v2)
        v3 <- call FIRSTCODE(v2)
LIR:    push v1
        push v2
        call B$SASS
        push v2
        call FIRSTCODE
MASM:   lea ax, ASC-LITERAL$D4
        lea si, ASC-LITERAL$D1+2
        push ax
        push si
        call far ptr B$SASS
        push si
        call far ptr FIRSTCODE
```

Module-level code has no `B$ENRA` frame, so its owned argument descriptor is
module data. Procedure call sites use a local descriptor counted in `BX` for
their runtime frame. This stays entirely in source semantic lowering and the
QB ABI envelope; MIR and the backend are unchanged.

The defect was the apparent QRender freeze after `colormap`: the debugger
stopped in `B$SLEP`, while the error log reported runtime error 5. A breakpoint
on `B$ERR_FC` led back to the `B$FASC` immediately following `B$FLEN` in fresh
`LS_LCHAR`. With expression materialization, fresh `COMMON` and fresh
`D_SURF` together in the otherwise-BC image render five frames and exit with
no `ERROR.LOG`; `BENCH.TXT` reports 153 polygons and 365 triangles.

Only after these rungs pass for the chosen runtime should multi-module project
substitution resume. qrender is an integration gate; it is not the instrument
used to discover a basic procedure-frame contract.

### 13. Module `$STATIC` numeric-array descriptor lifetime

`static-array-descriptor.bas` declares a shared LONG array and passes `values()`
to an external function. The descriptor is a separate read-only HIR data object
in `BC_CN`; its first field is a far relocation to the mutable array in
`BC_DATA`. The saved round is
`build/qbstages/static-array-descriptor-round`:

```text
input:  dim shared values(3) as long
        result = touch(values())
HIR:    VALUES -> mutable $data; VALUES$descriptor -> read-only data object
        descriptor[0] relocates far to VALUES; rank=1; flags=40h; width=4
MIR:    v1 <- address cell(global1:18[0:18])
        v2 <- call TOUCH(v1)
LIR:    lea v1, word ptr seg_2
        push v1
        call TOUCH
MASM:   lea ax, STATIC-ARRAY-DESCRIPTOR$D2
        push ax
        call far ptr TOUCH
```

The regression first failed while the descriptor shared `$data`: the emitted
EXE contained the right bytes, but the DOSBox debugger read eighteen zeroes at
the live descriptor after BASIC startup. Raw BC `SCREEN.OBJ` places
`H_FONT_CHAR()`'s descriptor in `BC_CN`, while its elements remain in
`BC_DATA`. After applying that general placement rule, a fresh `SCREEN.OBJ`
mixed build advances from `ugl` through `font`, map loading, and five rendered
frames. `BENCH.TXT` reports 153 polygons, 365 triangles, and camera leaf 115.
The full production stage dump is
`build/qbstages/screen-static-descriptor-round`.

### 14. `$DYNAMIC` bounded numeric arrays

`dynamic-bounded-array.bas` changes allocation mode before declaring a bounded
LONG array, REDIMs it, and writes its first element. The fail-first regression
initially produced a read-only static descriptor and only `B$RDIM`. After
recognizing the directive it exposed a second divergence from raw VBDOS
`MOD_TEX.OBJ`: fresh code used DGROUP, while VBDOS loads selector `AD+2` and
offset `AD+0Ah`.

The saved round is `build/qbstages/dynamic-bounded-array-round`:

```text
input:  '$dynamic / dim shared values(1) as long / redim values(3) / values(0)=7
HIR:    mutable VALUES$descriptor; call B$DDIM; call B$RDIM
MIR:    v4 <- load descriptor+2; v5 <- load descriptor+10
        v6 <- concat(v4, v5); element <- 7
LIR:    mov v4, word ptr [v3+2]; mov v5, word ptr [v3+10]
ASM:    mov ax,[bx+2]; mov cx,[bx+10]; ... mov dword ptr es:[bx],7
```

The production dump is `build/qbstages/mod-tex-split-huge-round`. Its
`MOD_LOAD_TEXTURES` now has the same descriptor-field shape as VBDOS. The
all-fresh DOSBox trace advances beyond `clip_nodes` through `textures`,
`mapclose`, `surfcache`, `backbuf`, and `colormap`; `LOAD.TXT` records a
complete 1.71-second load. No new `ERROR.LOG` or `ERRMEM.TXT` is produced.
The next boundary is after video initialization, before the five-frame run
writes `BENCH.TXT`.

### 15. Numeric array formal adjusted far pointer

`numeric-array-parameter.bas` passes a one-based dynamic LONG array to a SUB,
which stores `42` in element one. The fail-first regression showed that the
callee loaded a packed dword from descriptor offset zero. Raw VBDOS
`R_DRAW_WORLD` instead loads the allocation selector from `AD+2` and the
lower-bound-adjusted offset from `AD+0Ah`; using `AD+0` shifts nonzero-based
arrays and corrupts the first BSP walk.

The current saved isolated round is
`build/qbstages/numeric-array-split-far-round`:

```text
input:  '$dynamic / dim values(1 to 2) as long
        sub setFirst(values() as long): values(1) = 42
HIR:    load descriptor+2; load descriptor+10; add adjustedOffset, 4;
        concat selector, offset
MIR:    v2 <- load descriptor+2
        v3 <- load descriptor+10
        v4 <- v3 add 4
        v5 <- concat(v2, v4)
LIR:    mov v2, word ptr [v1+2]
        mov v3, word ptr [v1+10]
        add v4, 4
        push v2; push v4; pop v5
MASM:   mov ax, word ptr [si+2]
        mov bx, word ptr [si+10]
        add bx, 4
        ...
        mov dword ptr es:[bx], 42
```

The production comparison is
`build/qbstages/r-bsp-split-formal-round`; its numeric formal accesses now
have the same `+2/+0Ah` shape as the VBDOS listing. Numeric and UDT descriptors
use this rule whether owned or received as a formal. Dynamic STRING arrays
remain a separate near-descriptor case.

The corrected all-qbopt objects link. Per the listing-first diagnosis, this
round does not claim a new DOSBox result. Its linked footprint is 129,293
bytes of `BC_CODE` versus BC's 76,616; the +52,677-byte gap is recorded in
[footprint.md](footprint.md) and makes repeated descriptor reconstruction an
explicit optimization target.

### 16. Descriptor-base reuse with call invalidation

`numeric-array-base-reuse.bas` evaluates three elements, calls `INSPECT`, then
evaluates the same three elements again. The frontend emits one split data
base for each call-free region and deliberately reloads it after the call:

```text
input:  values(1) = values(2) + values(3)
        inspect values()
        values(1) = values(2) + values(3)
HIR:    base1 <- concat(load AD+2, load AD+10)
        element1/2/3 <- ptr_offset(base1, ...)
        call INSPECT(AD)
        base2 <- concat(load AD+2, load AD+10)
        element1/2/3 <- ptr_offset(base2, ...)
MIR:    two concat operations survive semantic, optimized, and physical MIR
LIR:    two selector/offset reconstructions around the call
MASM:   mov ax,[si+2] / mov dx,[si+10] appears twice, not six times
```

The saved stages are `build/qbstages/numeric-array-base-reuse-round`. The
all-qbopt link shrinks from 129,293 to 124,741 bytes of `BC_CODE`, a 4,552-byte
reduction, while retaining the corrected descriptor ABI. See
[footprint.md](footprint.md) for every module.

### 17. Descriptor field reuse (historical round)

At this point the frontend still believed rank-one indexing had to load the
lower bound once for every element. The semantic snapshot reused that
`AD+10h` metadata through a call-free region and invalidated it together with
the data base:

```text
input:  three accesses / call INSPECT / three accesses
HIR:    two lower-bound loads, two selector loads, two adjusted-offset loads
MIR:    one lower-bound value feeds all three pre-call subtractions;
        a second value feeds all three post-call subtractions
LIR:    repeated ptr_offset operations consume those shared values
MASM:   descriptor fields reload only after INSPECT
```

Stages are saved in `build/qbstages/numeric-array-field-reuse-round`. The
linked result falls from 124,741 to 120,181 bytes of `BC_CODE`, another 4,560
bytes, and remains 43,565 bytes above BC. D_SURF does not change, which rules
out descriptor-field reloads as the explanation for its largest remaining
gap.

The footprint instrument now also prices the helpers behind BC's calls. With
the successful all-BC control rather than the later failed-link MAP, complete
linked code is 255,047 bytes for BC and 292,210 for qbopt: a 37,163-byte,
14.6% gap. The BASIC-owned 56.9% gap and complete-code 14.6% gap remain paired
in every subsequent round.

The next listing comparison showed that this lower-bound read was itself
wrong: `AD+0Ah` is already adjusted. The saved numbers remain useful history,
but section 18 supersedes the interpretation and removes the read entirely.

### 18. Adjusted-base indexing and default split-far addressing

The raw VBDOS `D_SURF` listing makes the descriptor rule explicit. For
`ls_tab(30).epoch`, where the element width is 40 and `epoch` is at byte 38,
BC emits `mov bx,04D6h; add bx,[si+0Ah]`. It never reads the lower bound at
`AD+10h`: the adjusted offset has already incorporated it. The former HIR
subtracted that bound a second time and then normalized the packed result as a
huge pointer.

The fail-first regression records the actual symptom—D_SURF's one-based array
was addressed twice-adjusted and qrender remained black before its first
frame. The corrected isolated round is
`build/qbstages/numeric-array-split-far-round`:

```text
input:  '$dynamic / dim values(1 to 2) as long
        sub setFirst(values() as long): values(1) = 42
HIR:    bytes <- 1 * 4
        selector <- load AD+2; adjusted <- load AD+0Ah
        offset <- adjusted + bytes; pointer <- concat(selector, offset)
MIR:    offset <- adjusted add 4
        pointer <- concat(selector, offset)
        [pointer] <- 42
LIR:    mov selector,[AD+2]; mov offset,[AD+10]; add offset,4
MASM:   mov ax,[bx+2]; mov bx,[bx+10]; add bx,4
        mov dword ptr es:[bx],42
```

The complete D_SURF stages are in
`build/qbstages/d-surf-split-far-round`. `LS_SELFTEST` falls from 1,029 to 736
emitted instructions, and its optimized MIR falls from 40 to 19 remaining
`ptr_offset` operations. Across the linked program, BASIC-owned code falls
from 120,181 to 101,533 bytes. Complete linked code falls from 292,210 to
273,562 bytes, leaving a measured 18,515-byte (7.3%) gap over BC. The corrected
program still reaches `colormap` and remains black before its first frame, so
this closes one proven semantic defect but does not claim the integration gate.

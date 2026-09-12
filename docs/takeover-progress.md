# Takeover checkpoint — 2026-09-09

## 2026-09-10: select TEST for flag-only AND results

Lowering now emits a destination-free TEST for 16/32-bit register ANDs when
their scalar result has no reader and is not visible at the body exit.
Memory forms, partial writes and observed results retain AND. MIR remains
unchanged; instruction choice belongs to the backend.

The extra copy left by the preceding LICM improvement disappears:

```asm
; before              ; after
mov cx,bx             test bx,bx
and cx,bx             je otherArm
je otherArm
```

PDS IVWORD shrinks 1123 -> 1121 object bytes and removes one register copy
per iteration. The invariant load stays outside the loop. Both new width
tests fail before the change; 32 distinct lowering/induction checks pass,
including read-result, exit-visible and partial-write safeguards. IVWORD
still prints `34 0 11 37` plus DONE on all three linked compiler builds.

Before/after stages: `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-test-lowering-stages-tbnbvm8x`.
Runtime artifacts under the same temporary parent:
`qbopt-test-lowering-p-g2-kp6j8dtk`, `qbopt-test-lowering-q-O-sj5o8r7e`,
`qbopt-test-lowering-v-g3-ikyw3m5n`.

## 2026-09-10: retain branch tests without rejecting their invariant inputs

LICM previously rejected its entire invariant candidate sequence when a
flag result was needed inside the loop. It now retains those producers and
their dependent candidates, repeating until no flag crosses the loop edge;
independent input loads can still move. The original crossing check remains.

IVWORD's QB/PDS branch-condition load now executes once instead of ten times.
VBDOS uses a combined memory compare and is unchanged. Actual PDS assembly:

```asm
; before, inside loop       ; after, before loop
mov bx,[branchChoice]       mov bx,[branchChoice]
and bx,bx                   ; after, inside loop
je otherArm                 mov cx,bx
                            and cx,bx
                            je otherArm
```

This replaces a per-trip memory load with a register copy; it does not yet
produce the ideal TEST-only sequence. The PDS object grows 1121 -> 1123 bytes.
The extra copy is a remaining lowering opportunity, not omitted from the
reported result. The 56 focused induction/loop-motion checks pass, and all
three linked baseline/optimized IVWORD runs retain `34 0 11 37` plus DONE.
Both improved compiler cases fail their regression against the old pass.

Stage dumps: `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-licm-condition-stages-xy1bi73n`.
Runtime artifacts under the same temporary parent:
`qbopt-licm-condition-p-g2-c0bahk04`, `qbopt-licm-condition-q-O-b5huwj6x`,
`qbopt-licm-condition-v-g3-ou7pdnl8`.

## 2026-09-10: remove unobserved word-preservation dependencies in raise

The INTEGER version of IVARM kept QB/PDS's loop counter alive solely because
a narrow branch-condition load preserved an unused upper word. The raise now
removes these dependencies from 16-bit loads/copies only after tracing wide
and unknown reads, phi/merge edges and body-exit values. Actual low-word
operands remain uses. No optimizer is allowed to ignore a real dependency.

The existing exit-register observation analysis moved unchanged into the
frontend module; legacy optimization liveness delegates to that same source.
This is not complete elimination of partial-write MIR: observed upper bits
and other operations still retain their existing representation.

PDS IVWORD now has the same recurrence-controlled tail as IVARM:

```asm
; before                  ; after
add ax,3                  add ax,3
inc bx
cmp bx,10                 cmp ax,37
jle loopBody              jne loopBody
```

The counter initializer disappears and its exit store becomes constant 11;
the PDS object remains 1121 bytes. QB gains the same elimination; VBDOS
already eliminated this counter. IVWORD prints `34 0 11 37` plus DONE in
all three linked baseline/optimized runs. HARR, PRESSX and LNGMIX also pass
on all three compilers. The two newly improved compiler cases and direct
normalization test fail with normalization disabled; 35 focused tests pass.

Before/after stage dumps:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-ivword-stages-g200nme1`.
Runtime artifacts under the same temporary parent: `qbopt-ivword-p-g2-sqn4jp6q`,
`qbopt-ivword-q-O-p5a2tbgx`, `qbopt-ivword-v-g3-g7l19eju`, and
`qbopt-words-gate-p-g2-jdjpzu56`, `qbopt-words-gate-q-O-4fx45at8`,
`qbopt-words-gate-v-g3-hht51ofi`.

## 2026-09-10: reuse induction variables across internal branches

Removed IndVarSimplify's two-block-only restriction. The existing proof
already requires a single unconditional latch, header-only exit, known
non-wrapping trip count, a usable recurrence with sufficient modular period,
and no intermediate observation of the removed counter. Internal branches
do not invalidate that proof.

IVARM, genuine QB/PDS/VBDOS output, conditionally stores a value that advances
by three over ten iterations. PDS's loop tail changes as follows:

```asm
; before                     ; after
add ax,3                     add ax,3
inc bx
cmp bx,10                    cmp ax,37
jle loopBody                 jne loopBody
; exit: store bx             ; exit: store constant 11
```

One increment per iteration and the redundant counter initialization are
removed. The final counter store becomes immediate; the complete PDS object
remains 1127 bytes. All three linked baseline/optimized programs print
`34 0 11 37` and DONE. Three emitted-code regressions fail against the old
pass; all 18 induction-simplification tests pass, including observed-counter,
zero-trip, wrapping-exit and short-period safeguards.

The first stage difference is `s28-mir-r02-strength.txt`; full before/after
dumps are in `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-ivarm-stages-xl1hmqai`.
Runtime artifacts have the same temporary parent and names
`qbopt-ivarm-runtime-p-g2-1u4y1yd3`, `qbopt-ivarm-runtime-q-O-bk07y47p`,
and `qbopt-ivarm-runtime-v-g3-l4oiu248`.

An INTEGER branch condition instead retains a partial-write merge dependency
on the counter in raised MIR and is still refused. That is a remaining raise
normalization issue, not permission to ignore uses inside IndVarSimplify.

## 2026-09-10: exclude unreachable edges from loop analysis

Dominators previously intersected unreachable predecessors into live joins;
a dead edge erased real dominance, while disconnected cycles retained all
blocks as dominators. Loop discovery could miss a live loop or invent dead
ones, and dominance frontiers invented phi sites. No runtime miscompile was
demonstrated from this defect.

Dominance now starts from entry reachability. Natural-loop reverse walks,
irreducibility and dominance frontiers use only reachable edges, while
unreachable blocks retain empty analysis results. No machine-specific fact
was added to MIR analysis.

Four regressions failed against the original analysis. Fourteen focused CFG
checks and 59 LCSSA/loop-motion/GVN consumer checks pass. All stage files for
PDS RNGARM compare identically before/after, including the 1060-byte emitted
object listing: this fixes analysis correctness, not a claimed speedup.
Stage evidence: `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-dominance-stages-vzhl0hoj`.

## 2026-09-10: register CMPORD's complete reference

The source's four LONG pairs are all strictly ascending. Each yields the
same six forward/reverse Boolean pairs; all eight initial numeric stores
remain before printing. The required 24 three-call rows, DONE and termination
give **8*6 + 24*3*(6+20) + 26 + 20 = 1966** ranking units. The derivation
and abstract assembly listing are in `docs/targets.md`.

QB/PDS/VBDOS independently emit those eight constants and the expected call
counts; none of the eight numeric addresses escapes. Each linked baseline
and optimized executable passes all 24 golden rows. Three fail-first target
tests plus three event-reference safeguards pass. No compiler code changed:
before and after both retain the same initial stores and constant-argument
printing sequence; no assembly-size or speedup claim is made.

The 15 CMPORD rows were rescored, not the full corpus again: twelve ordinary
builds are 1.00x; three event builds remain provisional. Combined with the
preceding complete snapshot, coverage is **301 comparable/passing, 105
provisional, 81 without targets**. FPCSEX and FPDEEP remain provisional;
their exception/observability obligations were not relaxed.

Runtime artifacts under `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T`:
`qbopt-cmpord-reference-p-g2-jp_tsoey`,
`qbopt-cmpord-reference-q-O-vw_9s607`,
`qbopt-cmpord-reference-v-g3-wvwjjj81`.

## 2026-09-10: full scoreboard checkpoint

At b126fb4, all 487 `fixtures/omf` objects were measured once: 289 comparable
rows within 1.5x, none above it, 102 provisional and 96 without targets.
No row was unmeasured. Worst comparable ratio is IVCHAN's 1.36x. The gate
still returns 1; regression fixtures outside that directory and unfinished
architecture requirements are not included in these coverage totals.

[Current coverage](target-coverage-current.md) now replaces the stale status
summary, with historical evidence retained separately. FPCSEX costs
4462/4467/4452 and FPDEEP 1777/1572/1777 for PDS/QB/VBDOS, both still
provisional. Next priorities are validated floating/event references and the
eight uncovered source programs, alongside remaining compiler work.
No code changed in this checkpoint: before/after assembly is identical.

## 2026-09-10: inductive dynamic-array extent proofs

`frontend/arrayfacts.py` now joins numeric intervals, widens growing bounds
at loop headers, and refines them on comparison edges. A direct store's SSA
binding lets the same guard refine its memory copy. Each array store must
first fit its active allocation; only then can it preserve descriptor and
counter facts for the next iteration. Annotations use converged states, not
the optimistic first iteration. Exhaustion still adds no facts.

HUGERG extends the two-dimensional huge-array example to **197 iterations**.
The old exact walker exhausted its 10,000-operation budget. The inductive
proof succeeds within **1,000 operations** on QB, PDS and VBDOS, enabling
descriptor hoisting and two address recurrences. PDS excerpts:

```asm
; before, inside the loop: reconstruct index and address
mov dx,[items.dimensionCount]
movsx edx,dx
and edx,0FFFFh
imul ebx,edx
add ebx,ecx
shl ebx,1
mov edx,[items.pointer]
; huge-pointer normalization follows

; after: descriptor setup precedes the loop; EBX is the first address
push es
push ebx
pop si
pop es
mov [es:si],dx
pop es
; at the latch, after calculating the huge-pointer carry correction
add ebx,2
add ebx,ecx
; the second address advances by 402 bytes, with its own carry correction
```

Object sizes: PDS **1856 -> 1750**, QB **1867 -> 1743**, VBDOS
**1993 -> 1887**. Huge-pointer carry adjustment remains; no flat-pointer or
cycle-equivalence claim is made. All three baseline/optimized runs print
**456,457,789,790; DONE**. Final bytes match those runtime-verified objects.
The fixture uses the usual compiler switches plus `/AH`.

The focused array/extent/memory-join selection passes **61 tests**. Turning
range joins back into equality-only joins fails all three new proof cases.
Tests also reject unknown loop guards, out-of-allocation offsets, calls,
stale allocations and implicit width extension. Emitted-body checks require
descriptor loads to be absent from the loop.

Stages:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-hugerg-stages-zht1_3om`
(`before/s43-asm-emitted.txt`, `after/s75-asm-emitted.txt`). Runtime folders
under the same parent: `qbopt-hugerg-p-g2-bna872no`,
`qbopt-hugerg-q-O-2ffa9y41`, `qbopt-hugerg-v-g3-8dla5_j2`.
Runtime-sized extents, more precise call/lifetime effects, backend carry
costs and the rest of the architecture/target checklist remain unfinished.

## 2026-09-10: branch-scoped subscript ranges

`analysis/ranges.py` now refines signed integer intervals on comparison
edges and propagates tighter bounds through derived arithmetic. A bound
applies only where its dedicated successor dominates the use, not through
a join reachable from the other arm. Arithmetic TEST flags, partial flag
effects and unsupported comparisons do not establish subtraction bounds.

RNGARM iterates INDEX from 0 to 9 but writes a four-element word array only
under `index < 4`. Its offset is therefore 0..6 on that arm, not 0..18.
The array store cannot overwrite INDEX, allowing existing exit-store motion
to defer INDEX's write. PDS assembly:

```asm
; before: loop test, reached eleven times
mov [index],ax
cmp ax,9
jle body

; after: one write, with the final value 10
cmp ax,9
jle body
mov [index],ax
```

Object size stays **1060/1053/1195 bytes** for PDS/QB/VBDOS. This removes ten
executed counter stores, not static instruction bytes. All three baseline
and optimized runs print **28,7,10; DONE**. The guarded-loop and three real
fixture tests fail when edge refinement is disabled. The TEST-flags hazard
also failed before its guard was added.

The final edge-range/loop-motion/array-fact selection passes **60 tests**.
An earlier range/IndVar selection passed 55 and hit three existing FPDEEP
failures: unrolling removes the indexed accesses those tests expect.
All three also fail with edge refinement disabled; they were not weakened.

Stage dumps:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-rngarm-stages-d9pn5yw6`
(`before/s59-asm-emitted.txt`, `after/s75-asm-emitted.txt`). Runtime folders
under the same parent: `qbopt-rngarm-p-g2-4_495kzy`,
`qbopt-rngarm-q-O-vn84g4c9`, `qbopt-rngarm-v-g3-kgao5ak6`.

Dynamic-array induction remains a separate blocker: an unproven heap store
can clobber the counter reload, preventing recurrence recognition. The
solution needs an inductive allocation/range invariant, not an assumption
that the very access being proved is already in bounds.

## 2026-09-10: reuse element values through pointer phis

`loadjoins` now translates a whole-pointer phi separately on each incoming
edge before selecting the supplying load/store. MemorySSA checks the join
prefix against the original address and the incoming path against its
translated address. Unknown effects, overwritten memory and mismatched
pointer inputs still prevent reuse. No reads are inserted.

ARRPHI now keeps each branch's stored value across the join. PDS first join:

```asm
; before
push es
push ebx
pop bx
pop es
mov ax,[es:bx]
pop es
add ax,3

; after: AX already contains the value stored by either branch
add ax,3
```

Both element reloads disappear. Optimized object sizes: PDS **1580 -> 1562**,
QB **1560 -> 1539**, VBDOS **1712 -> 1694**. The prior array-fact regression
now checks the raised body, before optimization deliberately removes two of
its six accesses; separate emitted-body tests require all four stores and
zero element loads. Those three tests fail with phi translation disabled.

All three baseline/optimized executions print **10; 9; DONE**; **67 focused
tests pass**. Stage dumps are in
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-arrphi-value-stages-4r8gb9cu`
(`before/s59-asm-emitted.txt`, `after/s75-asm-emitted.txt`). Runtime artifacts
under the same temporary parent: `qbopt-arrphi-value-p-g2-y5s_7ao0`,
`qbopt-arrphi-value-q-O-agcmui5w`, `qbopt-arrphi-value-v-g3-ifdtxepb`.
No full-suite run or new timing ratio is claimed. General symbolic index
ranges, missing-path load insertion and the remaining roadmap stay open.

## 2026-09-10: keep array facts across unknown branches

`frontend/arrayfacts.py` meets known values, memory cells and allocation
extents across CFG predecessors. Unlike the exact-path proof, it need not
choose an unknown branch. Allocation lifetime and value width are explicit;
unknown calls, descriptor writes and cyclic allocations cannot establish
unsafe ownership. The existing finite-loop proof remains available.

ARRPHI uses two dynamic arrays and READ-supplied branch conditions. All six
constant-offset accesses are now proven within their allocations. Existing
CSE shares the computed pointer through a phi, removing the join's repeated
descriptor loads and huge-pointer arithmetic. The element load remains.

PDS first join, abbreviated only where marked:

```asm
; before
mov ax,[firstValues.lowerBound]
movsx eax,ax
mov ebx,2
sub ebx,eax
lea eax,[ebx+ebx]
mov ebx,[firstValues.pointer]
; normalize the huge pointer (shift/add sequence)
push es
push ebx
pop bx
pop es
mov ax,[es:bx]
pop es
add ax,3

; after: EBX carries the pointer computed on either branch
push es
push ebx
pop bx
pop es
mov ax,[es:bx]
pop es
add ax,3
```

Optimized object bytes (previous pipeline -> new pipeline): PDS
**1764 -> 1580**, QB **1744 -> 1560**, VBDOS **1896 -> 1712**.
These include relocation/debug records, not just instructions; no cycle
ratio is inferred. Baseline and optimized executions on all three compilers
print **10; 9; DONE**. The fixtures were compiled with the usual configuration
switches plus `/AH`. Final output bytes match the runtime-verified artifacts.

The scoped array/bounds/memory-join checks pass **47 tests**. Stage dumps:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-arrphi-stages-khvix8rz`
(`before/s43-asm-emitted.txt`, `after/s59-asm-emitted.txt`). Runtime artifacts:
`qbopt-arrphi-p-g2-n7u06p3o`, `qbopt-arrphi-q-O-final-js725ryt`, and
`qbopt-arrphi-v-g3-ixz5z4ef` under the same temporary parent.

Next: translate pointer phis for memory-load reuse, then generalize constant
offset proofs to induction ranges. The broader target and architecture goal
remains open.

## 2026-09-10: reuse memory values across joins; repair READ escape facts

`optimize/loadjoins.py` replaces a whole scalar load with a phi of the values
already loaded or stored on every predecessor. MemorySSA checks each edge's
memory version, including aliasing writes in the join prefix. No loads are
inserted. Partial values, stack-relative cells, floating operations and
providers requiring an unrepresented loop-exit phi are excluded.

MEMPHI exercises both branches with values supplied by READ. Each branch
stores a different integer to an array element; the next statement reloads
that element. After the pass, the branch result stays in AX:

```asm
; before, either branch has stored AX into the array
mov ax,[firstValues+2]
add ax,3
mov [answer],ax
; after
add ax,3
mov [answer],ax
```

Two array reloads disappear: PDS object **1280 -> 1264 bytes**, including
removed fixups; emitted instructions lose six bytes. Baseline and optimized
QB/PDS/VBDOS executions print **10; 9; DONE**. The read-destination checks
count memory reads, not MOV alone: VBDOS tests the variable directly with CMP.

The fixture also exposed a pre-existing miscompile: QB/PDS printed **10;10**
even with the new pass disabled. Forwarding reused the first branch condition
after a second READ. Escape analysis had only recognized an immediately
pushed address; BC inserts PUSH DS / POP ES / PUSH ES between MOV OFFSET and
PUSH register. It now follows that register until the push, a write to any
overlapping register part, or control transfer. Numeric-value argument
exclusions still refer to the actual consuming push.

MemorySSA also now treats strict floating operations and FCHECK as unknown
memory definitions: an observable exception must not preserve caller-memory
facts across a potential handler. These safety checks have fail-first tests.

The final memory/escape checks pass **53/53**. A 24-object comparison leaves
21 byte-identical; PRESSX grows eight object bytes in each compiler because
PRINT can no longer be assumed disjoint from all program data. It reloads
the result for the following numeric PRINT instead of carrying it across
the preceding call. All three PRESSX executions pass, and its verified model
cost remains within target: **627 / 508 = 1.23x**.

Stages: `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-memphi-fixed-p-g2-h5p97qgu`.
Remaining array blocker: `raising_array_bounds.proven` currently requires one
allocation and known branch outcomes. Unknown branches lose allocation
disjointness, so dynamic-array descriptor/address reloads still prevent this
reuse. Long huge-array consumers also retain unsupported HARY sites in the
exploratory example. Neither limitation is counted as optimized or complete.

## 2026-09-10: scalar partial redundancy on dedicated incoming edges

GVN can now supply a missing scalar computation on an unconditional incoming
edge when another path already computed it. Inputs must dominate insertion;
live conditions, loop-boundary crossings and critical edges are excluded.
CSE must stabilize first, so temporary differences in value representation
do not cause insertion where an existing provider will become visible.

GVNPRE exercises both branches with runtime inputs. Its taken branch used to
square twice; it now keeps the first square. The other path still squares
once. PDS object size stays **1169 bytes**, static multiplies stay **2**,
and executed multiplies over the two trials fall **3 -> 2**:

```asm
; before, square-producing branch
mov ebx,eax
imul ebx,eax
add ebx,1
mov [answer],ebx
jmp commonSquare
; other branch computes answer, then falls through
commonSquare:
imul eax,eax
mov [square],eax

; after, square-producing branch
imul eax,eax
mov ebx,eax
add ebx,1
mov [answer],ebx
jmp saveSquare
; other branch computes answer, then squares only on its own edge
imul eax,eax
saveSquare:
mov [square],eax
```

Both paths print **7,36; 37,36; DONE** on QB, PDS and VBDOS. The first
implementation printed **37,1296**: the inserted operation used the join's
address, so a branch to the join also executed that operation. Stage dumps
showed correct MIR/LIR block membership but the wrong emitted jump target.
The occurrence now belongs to the predecessor; real-object regressions
check that the jump skips the edge computation on all three compilers.
Twenty-one BOOLS/FLAGS/NOTS/ARITH/NEGNOT/PRESSX/FPCSEX outputs are unchanged.
Stages: `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-gvnpre-fixed-stages-51lp3hmh`.

This is scalar PRE on dedicated edges, not completion of GVN-PRE or the goal.
Memory expressions, critical edges and target profitability remain.

## 2026-09-10: translate GVN expressions through input phis

GVN now matches each join expression using that edge's incoming phi values.
Translation is simultaneous, so swapped loop inputs are not recursively
substituted. The pass still inserts no arithmetic and stays machine-independent.

GVNPHI reads inputs and squares `inputValue + 1` on one branch and
`inputValue + 2` on the other, then squares the selected value again at the
join. Both paths retain their own square:

```asm
; before, at the join
imul eax,eax
mov [square],eax
; after
mov [square],eax
```

Static multiplies **3 -> 2**, executed multiplies **2 -> 1 per iteration**;
PDS object **1200 -> 1195 bytes**. Both paths print **48,49; 37,36; DONE**
under QB, PDS and VBDOS. MIR and all three emitted-code regressions failed
without phi translation. Stage dumps:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-gvnphi-stages-vj75d8jt`.
Dedicated-edge insertion was added in the follow-up above; memory PRE and
profitability remain open.

## 2026-09-10: eliminate scalar redundancy across joins

`optimize/gvn.py` extends CSE beyond a single dominating provider. If each
incoming edge already computes the same scalar expression, a fresh phi joins
those values and replaces the repeated operation. Keys use semantic kinds,
operands and widths, not instruction names. The pass inserts no arithmetic,
does not read register origins, preserves byte ownership and leaves floating,
memory, live flags and loop-exit crossings requiring LCSSA unchanged.

The real GVNJN fixture reads inputs, computes a square in either branch,
adjusts the branch answer, and asks for the square again at the join. Both
branches are exercised. Static multiplies fall **3 -> 2**, and executed
multiplies **2 -> 1 per iteration**; allocation coalesces the phi without
adding join moves. PDS optimized object size is **1171 -> 1166 bytes**.

```asm
; before, after either branch computed its adjusted answer
join:
    imul eax,eax
    mov [square],eax

; after, both branches keep the original square in EAX
join:
    mov [square],eax
```

The MIR and three emitted-code regressions failed first; 39 focused CSE/GVN
checks pass. All three compilers build/link successfully and print
**35,36; 37,36; DONE**. Twenty-one existing BOOLS, FLAGS, NOTS, ARITH, NEGNOT,
PRESSX and FPCSEX objects emit byte-identical output with the new step.
Stages: `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-gvnjn-stages-dhpr1e8y`.

This is full redundancy elimination at joins, not complete PRE: missing-edge
insertion, memory expressions and profitability remain. Phi translation was
added in the follow-up above.
FPCSEX remains at 4462 model units on PDS with an invalid provisional
denominator; this integer optimization does not establish floating progress.

## 2026-09-10: forward values through whole-pointer memory

MemorySSA now skips writes at disjoint constant offsets from a shared pointer
base when both accesses are proven inside the same allocation. The new
`analysis/pointerfacts.py` derives relations from current SSA copies and
PTR_OFFSET operations; no machine encodings or stale address annotations
enter MIR. Partial overlap, unrelated roots, missing ownership, partial copies
and unknown calls remain conservative.

Constant propagation consults exact dominating stores for pointer loads.
The existing forwarding consumer now also accepts whole-pointer loads/stores
and forwards nonconstant SSA values. Static-cell propagation remains intact.

NDMAX's first PRINT no longer reloads 11 after storing 22 six bytes away:

```asm
; before
push es
push eax
pop bx
pop es
mov ax,[es:bx]
pop es
push ax
call B$PEI2

; after
push 0Bh
call B$PEI2
```

PDS optimized object size: **1244 -> 1233 bytes**. The constant, nonconstant
and three real-object regressions failed first. Pointer/MemorySSA focused
tests pass 43 cases. All three `/AH` DOS builds print **11, 22, DONE**; final
objects are byte-identical to those runtime-validated outputs.
Stage dumps: `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-pointer-constant-stages-kmw2de4b`.
This extends the shared alias/MemorySSA foundation; it is not full GVN-PRE
or a verified target-ratio result.

## 2026-09-10: reuse allocation proofs during checked-array recognition

Checked recognition now runs to a fixed point on the same body, with each
additional round required to remove a HARY call. The existing exact path
proof establishes that a native store targets array data, not its descriptor;
feeding that fact into the next round preserves the descriptor constants.
No new alias assumption is introduced.

NDMAX drops from **3 -> 1 HARY calls**, and the PDS optimized object shrinks
**1388 -> 1244 bytes**. The final access remains checked because the preceding
print call invalidates the descriptor facts. All three real-object regressions
failed first; 18 focused checked cases pass. PDS, QB and VBDOS `/AH` DOS runs
all print **11, 22, DONE**, with successful compilation and linking.

```asm
; before: sixty indices and rank pushed for the first PRINT
call B$HARY
push word [es:bx]
call B$PEI2

; after: EAX still holds the allocation's whole pointer
push es
push eax
pop bx
pop es
mov ax,[es:bx]
pop es
push ax
call B$PEI2
```

The second store uses native huge-pointer offset arithmetic (+6 bytes).
The remaining load should eventually forward the first store's constant;
that is a separate memory-optimization opportunity, not a completed result.
All-stage dumps: `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-checked-fixedpoint-stages-rx3gy39p`.

## 2026-09-10: eliminate proven checked dynamic-array accesses

With `--bounds-checks`, raising now replaces HARY only when every captured
index is within its dimension's current bounds and rank, element width and
descriptor features agree. A flattened offset inside the allocation is not
enough: an individual dimension can still be out of range. Unknown facts
retain the call. This is check elimination, not loop-check hoisting.

NDMAX's first access checks sixty constant zero indices unnecessarily. PDS
optimized object size drops **1481 -> 1388 bytes**, with HARY calls **4 -> 3**.
The three remaining calls are not proven safe. Relevant emitted assembly:

```asm
; before: sixty zero indices pushed, then
push 3Ch
mov ax,descriptor
mov bx,ax
call B$HARY
mov word [es:bx],0Bh

; after: the zero offset folds away
mov eax,[descriptor]
push es
push eax
pop bx
pop es
mov word [es:bx],0Bh
pop es
```

This exposed a backend refusal: pointer ABI setup depended on PTR_OFFSET
surviving optimization. Whole-pointer memory accesses now request it too.
The three zero-only emission regressions failed before that correction.
The original array module passed 47 tests; the expanded focused checked
cases pass 15. All three `/AH` compiler builds link and produce the original
**11, 22, DONE** with checks enabled. `/AH` is necessary for this fixture;
without it BC itself rejects the expression. The golden includes BASIC's
leading sign space; existing run artifacts were rejudged, not rerun.

All-stage before/after dumps:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-checked-ndmax-stages-agg67_cw`
(`before` and `fixed`). Runtime artifacts are under
`qbopt-checked-native-{p-g2-i9f9e89y,q-O-ihs67yxo,v-g3-kivd3c6x}` in the same
temporary root. No hardware timing or target-ratio improvement is claimed.

## 2026-09-10: allocate straight-line floating regions by CFG, not list adjacency

The remaining FPCSE initial stores are deliberately protected by FCHECK:
an incoming pending exception can observe the earlier state. The existing
dead-store analysis already propagates across blocks; deleting that barrier
would be a correctness regression, not the missing cross-block optimization.

The floating allocator had a separate representation-dependent restriction:
it carried a shared value across a unique straight-line edge only if the
two blocks were adjacent in the body tuple. Reversed or separated storage
order refused the same valid CFG. Allocation now schedules those regions
using unique predecessor/successor edges and restores the original block
order afterward. It changes neither control flow nor floating evaluation,
conversion, or exception ordering. Forks, joins, entry backedges, floating
phis and unknown-call crossings retain their existing restrictions.

Before: the reordered shared-product/quotient reproducer refused allocation.
After: the first block uses `fld; fld st(0); fmul; fstp`, retaining the shared
value for the successor's `fdiv; fstp`. Both new ordering cases failed first;
the floating allocator module passes 43 tests. FPCSE, FPDEEP and FPCSEX
outputs are byte-identical before/after across the three primary compilers.
This is an allocator capability improvement, not a measured suite speedup.
General floating phi/loop allocation and justified strict-FP reuse remain open.

## 2026-09-10: lay removed-loop bodies out in execution order

Refreshed the full target ranking once. FPCSE PDS/VBDOS remained 167/98;
FPDEEP PDS/VBDOS remained 1803/1086, while QB FPDEEP was already 1604/1086.
Provisional event/input references and missing targets still prevent a
completion claim. This is a model ranking, not a runtime timing suite.

The emitter retained source-address block order after removing FPCSE's loop:
entry jumped to the old header, which jumped back to the old body, which
jumped forward to printing. Authoritative ordered bodies now place a complete
acyclic single-successor chain in execution order. The assembler removes the
resulting fallthrough jumps. Branching, cycles, disconnected/external edges,
legacy ordering and embedded data keep their previous placement. Trailing
data outside a body's instruction footprint does not prevent placement.

Before: `mov ax,1; jmp header; ... header: mov [i],ax; jmp body`.
After: `mov ax,1; mov [i],ax; wait; ...` with the same executed effects.
FPCSE PDS/VBDOS cost **167 -> 161**; QB remains **147**.
FPDEEP PDS/QB/VBDOS **1803/1604/1803 -> 1797/1598/1797**.
All six runtime outputs match originals. FPCSE's real-object no-JMP
regression failed first. Layout/emission tests: 4400 passed, 18 failures
also reproduced with unmodified HEAD's layout; five extra placement guards
pass. No test was weakened. Dumps and DOS evidence:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-linear-layout-0p5kai7p`.
The remaining floating-point gaps are not closed by this placement change.

## 2026-09-10: propagate pointer displacement constants and remove zero steps

Constant operand propagation omitted PTR_OFFSET. NDMAX retained a known-zero
displacement as a Held value and lowered it through full huge-pointer
normalization. PTR_OFFSET now accepts width-proven displacement constants
without swapping operands or treating a pointer as an integer displacement.
The machine-independent identity `ptr_offset(p, 0) = p` then becomes COPY.
NDMAX's first fold dump shows `v604 := ptr_offset v603, 0`; the algebraic
dump shows `v604 := v603` instead.

Object bytes PDS/QB/VBDOS after this change:
NDMAX **4211/4202/4345** (PDS was 4292 immediately before);
NDARR **2518/2510/2650 -> 2417/2409/2543**;
HUGELP **1735/1727/1872 -> 1734/1719/1871**.
All nine optimized runtime outputs match their originals. The three real
zero-displacement regressions failed first. Focused algebraic/constants/
pointer tests: 1156 passed; four failures (three ADDRM shape counts and one
NBODY negation count) also reproduce with the unmodified baseline functions.
No assertions were weakened. Stage and DOS artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-pointer-constants-uok9o47h`.

Do not infer a zero base offset from a small allocation: Microsoft's
`runtime/rt/dynamic.asm` initializes FHD_oData to sizeof(AHD), while huge
allocations of at least 64K use 64K modulo element size. Allocation bounds
alone therefore do not establish the backend's no-segment-crossing proof.

## 2026-09-10: correct packed-pointer carries without rebuilding both halves

Backend pointer lowering now computes a direct packed addition, then corrects
its unit selector carry to the runtime selector stride. If `pages` is the
carry from low-word-plus-displacement, the correction is
`((pages << huge_shift) - pages) << 16`. This removes one arithmetic operation
and reduces the temporary register/move pressure without assuming DOS's shift
or dropping boundary normalization. MIR remains unchanged.

NDARR object bytes PDS/QB/VBDOS: **2607/2599/2740 -> 2518/2510/2650**.
HUGELP: **1801/1794/1937 -> 1735/1727/1872**. Weighted cost estimates:
NDARR 117239/117257/117251 -> 106346/106364/106348;
HUGELP 2457/2363/2457 -> 2365/2263/2365. These are ranking-model values,
not hardware timing measurements. Both programs' actual DOS outputs match
their originals on all three compilers.

The nine-operation regression failed before the change. Focused pointer,
huge-array and induction tests: 111 passed. An additional independent
selector/offset oracle covers all 16 supported shifts, wrap and borrow
boundaries; the final pointer module passes 40 tests.
All stage dumps and runtime evidence:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-pointer-correction-7qw_x23l`.
Remaining: avoid normalization only when allocation/range facts prove no
boundary crossing, and reduce segment materialization around accesses.

## 2026-09-10: carry multidimensional loop pointers across iterations

Induction analysis now recognizes `i OR i` and `i AND i` as zero tests,
and preserves the recurrence through their unchanged value result (QB uses
that result where PDS/VBDOS use the input). XOR and masking by another
operand are not equivalent. Counter substitution shares the bound recognizer
instead of assuming every accepted test was SUB. Its existing use checks
also protect a logical operation's separately observed value result.

NDARR's inner MIR previously extended the row index, subtracted its lower
bound, added the enclosing-loop offset, multiplied by two and rebuilt the
pointer. It now stores through a loop-carried pointer and advances it by two;
the enclosing loops advance pointers by four and twelve respectively.
All three real-fixture pointer regressions failed before the change.
NDARR and HUGELP outputs match their originals on all three compilers.

This is not yet ideal machine code: huge-pointer advancement still performs
segment normalization. NDARR object bytes grow from 2446/2438/2592 to
2607/2599/2740 (PDS/QB/VBDOS). The existing weighted model decreases from
129133/129151/146577 to 117239/117257/117251; these are model rankings,
not actual trip-count timing measurements. Assembly confirms less inner-loop
address recomputation but repeated normalization remains a backend gap.

Focused induction/range run: 104 passed; three FPDEEP range tests also fail
on unmodified HEAD because their expected indexed loads have already gone.
Those tests were not weakened. Five logical-bound acceptance/rejection
cases pass separately. Stage and DOS evidence:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-ndarr-induction-vljjjibp`.

## 2026-09-10: prove multidimensional loops with logical zero tests

NDARR's inner loop uses a logical-result zero test rather than COMPARE.
The exact frontend path interpreter now records known 16-bit AND/OR/XOR
flags as comparison with zero. Unknown results and unsupported widths still
prove nothing; flag-free native address arithmetic does not require this rule.
The resulting checked element-store extent preserves descriptor facts across
the loop. Adjacent raise/fold dumps show the nine-dimensional descriptor
loads becoming constants and their address arithmetic simplifying.

Object bytes, previous/current: PDS **3256 -> 2446**, QB **3250 -> 2438**,
VBDOS **3384 -> 2592**. These are sizes, not execution-time measurements.
NDMAX stays unchanged at 4317/4308/4451 bytes. Both programs match original
runtime output on all three compilers: NDARR 1,12,2 and NDMAX 11,22, then DONE.
The three extent regressions failed before the fix. The two focused array
modules passed 54 tests; the added unknown-logical-condition rejection and
three positive cases then passed together (4 tests).
Stage dumps and runtime evidence:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-ndarr-extent-xezymida`.
This bounded concrete proof is not general symbolic range analysis; that
remains necessary for variable-trip loops and broader induction optimization.

## 2026-09-10: preserve constants after a runtime call becomes native MIR

`consts._kills` used the original call-address map without checking the
current operation. Every scalar operation expanded at HARY's old address
therefore discarded the allocation's dimension facts again. It now applies
call invalidation only to a current CALL. Real calls and aliasing stores
still invalidate facts through the existing rules.

The first fold dump at NDMAX's old HARY site changes, for example:

```text
before: v68 := lower-bound load; v69 := sign_extend v68; v70 := 0 - v69
after:  v68 := 0;                v69 := 0;               v70 := 0
before: v75 := dimension load;   ...;                    v78 := v70 * v77
after:  v75 := 1;                ...;                    v78 := 0
```

NDMAX object bytes PDS/QB/VBDOS: **9692/9683/9826 -> 4317/4308/4451**,
5375 bytes removed in each. NDARR is unchanged; its loop-carried memory
facts are a separate remaining opportunity. These are object sizes, not
hardware timings or target ratios.

All three real-object fact regressions failed before the fix. The focused
array, constant-call and constant-cell modules pass **62 tests**. NDARR and
NDMAX match original DOS output on all three compilers (1,12,2 and 11,22),
with successful links. Dumps and runtime output:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-ndarrays-facts-1hb839sh`.

## 2026-09-10: remove the invented eight-dimension array limit

Microsoft's `work/ms/msdos_60/45/qb/ir/prsid.asm` defines `MAXDIM EQU 60d`
as the BASCOM-shared limit. `runtime/rt/dynamic.asm` processes HARY's
indices from last to first without an eight-dimensional restriction.
Both native descriptor readers now accept 1–60 dimensions.

New real compiler fixtures, built with each primary configuration plus
`/AH`: NDARR uses nine dimensions, unequal extents and nonzero/negative
lower bounds in nested loops; NDMAX exercises 60 dimensions. All six
fixtures compiled with zero severe errors. Without `/AH`, the older
compilers report expression complexity errors for these shapes; those
failed compilation outputs are not fixtures.

Before: both programs retained HARY and unchecked emission was refused.
After: every HARY site raises to scalar address arithmetic and whole
pointers. All three compilers' original/native DOS results agree: NDARR
prints **1, 12, 2**; NDMAX prints **11, 22**; both finish with DONE.

The 60-dimensional expansion exposed an independent OMF writer defect:
one expanded original operation exceeded a LEDATA record, but only original
instruction locations were candidates for splitting. Records may split
instructions (BC already does); they may not split a FIXUPP field. The
writer now falls back to a byte boundary outside every relocation field.
The regression first failed at raising for all six fixtures, then at
record emission for all three NDMAX fixtures, before the respective fixes.
The focused array/relocation modules pass **2492 tests in 20 seconds**.

This is a support/correctness milestone, not good final code quality:
NDARR objects grow from roughly 1.4 KB to 3.3 KB; NDMAX from 1.4–1.9 KB
to roughly 9.7 KB. Known singleton dimensions should simplify far more.
Stage dumps and successful linked runs are under
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-ndarrays-native-tcshx93d`.

## 2026-09-10: select immediate arguments without temporary registers

Lowering folds an adjacent single-use immediate definition into its push.
The combined instruction retains the definition's relocation owner and
claims both original byte spans. Other readers (including phi inputs),
implicit requirements, width mismatches and separated byte ownership keep
the original sequence. This is backend instruction selection, not a
machine-specific MIR optimization.

QB FPCSE before/after, applied to both printed string descriptors:

```asm
; before                       ; after
mov ax,offset descriptor       push offset descriptor
push ax
call B$PSSD                    call B$PSSD
```

Removing the last AX definition also exposed an already-dead inserted
spill/copy chain. Allocation now removes unused inserted copies and owned
reloads before physical identity-copy deletion loses their virtual use
graph. Opaque instructions stop this cleanup; source loads, live values,
original byte-owning instructions and relocation owners are retained.

QB FPCSE cost **151 -> 147**, exactly **1.50x** the 98-cost reference;
object **819 -> 817 bytes**. PDS/VBDOS FPCSE remain **167/167** and outside
target. PROCS shrinks by **6/20/3 bytes** on PDS/QB/VBDOS; QB LNGMIX by
four bytes. No full-goal completion is claimed.

The emitted QB argument regression failed first. The focused combined
run passed 123 tests; the final explicit opaque-use guard also passed
(12 argument tests). FPCSE, PROCS and LNGMIX run identically to originals
on all three compilers, with successful links. Final outputs were compared
byte-for-byte to these runtime-validated artifacts after adding the
conservative opaque-instruction guard, avoiding unchanged DOS reruns.
Stage dumps and runtime output:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-direct-args-final-mr0mwvmt`.

## 2026-09-10: combine adjacent constant argument pushes

The backend now combines adjacent unrelocated word-immediate pushes into
one dword-immediate push. The second word occupies the low half, so stack
bytes and the total four-byte adjustment are unchanged. MIR remains
machine-independent. Relocations, instruction/block boundaries, nonadjacent
byte ownership and implicit operand requirements prevent fusion.

FPCSE before/after:

```asm
; before                       ; after
push 43F3h                     pushd 43F3C000h
push C000h
call B$PER4                    call B$PER4
```

PDS modeled cost **173 -> 167**, QB **157 -> 151**, VBDOS unchanged at
167 (already a wide push). Object sizes are unchanged: six encoded bytes
in either form. These remain above the 98-cost reference; no completion
claim. Current FPDEEP costs are PDS/QB/VBDOS **1803/1720/1803** and are
unchanged by push fusion.

The emitted-code regression failed before fusion. All 102 peephole tests
pass, including byte-order, signed constants, four-byte stack adjustment
and boundary guards. FPCSE and all eleven FPDEEP answers match original
DOS executions on each of the three compilers; links succeeded.
All stage dumps and runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-push-pairs-tr0joga6`.

## 2026-09-10: release empty allocator frame reservations

The backend tags its synthetic prologue/epilogue adjustments and removes
them together after peephole cleanup if no added frame storage remains.
Original stack adjustments are not tagged or removed. Live frame cells,
materialized frame addresses, unknown addressing and opaque instructions
keep the reservation; this does not compact partially used frames.

QB FPCSE before: `sub sp,2 / wait / ...`; after: `wait / ...`.
The spill reload was already removed in the preceding commit. Modeled
cost **159 -> 157**, object **822 -> 819 bytes**. PDS and VBDOS FPCSE
remain byte-identical. The emitted regression failed first on `sub sp,2`.
The focused peephole/prologue checks pass; original and optimized QB DOS
outputs both remain `S= 487.5` and `DONE`, with successful links.
Stage dumps and runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-empty-frame-28n57_c5`.
FPCSE is still **1.60x**, not within the target; removing unread program
stores needs a real visibility/lifetime proof, not a noreturn assumption.

## 2026-09-10: remove dead allocator reloads after physical assignment

The spiller now marks its own reloads in LIR. The physical dead-move
peephole may remove these when all destination byte lanes are overwritten
before use. Unmarked memory reads remain observable; merely using a frame
address is not proof that a load belongs to the allocator.

QB FPCSE before/after (descriptor relocation shown symbolically):

```asm
; before                       ; after
call B$PER4                    call B$PER4
mov ax,[bp-2]
mov ax,descriptorDone           mov ax,descriptorDone
push ax                        push ax
```

Modeled cost **165 -> 159**, object **825 -> 822 bytes**. PDS and VBDOS
FPCSE objects are byte-identical. The unused two-byte frame reservation
still remains; removing it requires recomputing frame use, not guessing
from this one load. FPCSE remains above the 98-cost reference target.

The emitted-code regression failed before the fix. All 93 peephole tests
pass, including ownership and live-read guards. QB original and optimized
DOS runs both print `S= 487.5` and `DONE`; linking succeeds. All pass dumps
and runtime artifacts are under
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-dead-reload-i79b8ssf`.
Adjacent prologue/peephole dumps show exactly the one reload removed.

## 2026-09-10: fix noreturn being mistaken for restricted memory access

Investigating terminal dead stores exposed an unsound frontend assumption:
`mir._narrowed` narrowed ANY memory effects solely because a runtime call
never returned. Nonreturning calls may still read memory or invoke user
handlers before exit. B$CENP specifically enters No RESUME handling when an
ON ERROR is active, and its existing contract explicitly says reads/writes
ANY. That effect must not become an exclusion for caller numeric data.

Removed the noreturn exception. Calls now require independently restricted
read and write contracts to receive the exclusion; B$CEND's established OWN
contract still qualifies. The regression checks the actual raised B$CENP
load effect against a caller store and fails with the old shortcut restored.
36 focused literal tests pass (the previously documented raw QB loop-exit
proof assertion remains excluded). FPCSE emitted objects are byte-identical
on all three primary compilers, so no unchanged DOS runs were repeated.
This prevents an invalid foundation for future terminal-store removal;
it is not a performance gain. A valid body/data lifetime proof is still needed.

## 2026-09-10: thread empty collapsed-loop blocks

MIR branch cleanup bypasses blocks containing only ownership markers and an
unconditional jump. It refuses phi-bearing intermediates or destinations,
real operations and cyclic redirections; explicit successor edges and branch
targets change together. Unreachable blocks retain their byte ranges but now
clear the obsolete instruction name: an old jump name otherwise lowered to
a one-byte NOP and prevented the emitter's existing fallthrough removal.

QB FPCSE before: entry jumps forward to an empty header, header jumps back
to the constant-result block, and that block jumps over the empty header to
PRINT. After: the same stores and calls in straight-line order, **no jumps**.
Ranking **171 -> 165**; PDS/VBDOS remain 173/167. Against 98, more work remains.
No register or encoding knowledge is needed in the MIR cleanup.

The emitted-code regression failed first. Five focused tests pass, including
phi, store and cycle guards. The broader floatfold/transform run had 64 passes,
two existing xfails and nine failures; all nine reproduce with the old
unreachable transform restored and threading disabled (42 passes, two xfails
in the baseline transform-only run). They were not weakened or reclassified.
FPCSE and BOOLS match original DOS output on all three primary compilers:
487.5 and T=2, respectively, then DONE; all runs finished normally.
Stage and runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-threaded-final-8d_44awd`.

## 2026-09-10: carry completed FP checks across integer-only edges

The MIR check simplifier now uses a conservative CFG must-analysis, rather
than resetting a completed observation at every block. Every predecessor
must have completed a check; floating operations, calls, opaque effects,
stack-bearing operations and unrecognized kinds invalidate the fact.
Integer comparisons and branches do not create pending FP exceptions.
Entry remains unproved even when a backedge has a check. No machine state
or register names enter the optimization.

QB FPCSE before: entry WAIT, s=0, i=1, an initial counter store, then a
second WAIT in the collapsed loop before final constant stores. After:
entry WAIT remains; the second WAIT disappears, allowing existing dead-store
and dead-value passes to remove the initial s and counter work. The printed
487.5 constant, final stores and calls are unchanged.
Ranking cost **190 -> 171**, object **857 -> 831 bytes**. Target stays 98
(1.74x, still not complete). PDS and VBDOS output objects are byte-identical
to the previous implementation.

21 focused tests pass, including fail-first agreeing-edge coverage and
negative call/FP-path and first-loop-iteration guards. QB original/optimized
DOS output matches `S= 487.5` and DONE; the run finished normally and both
links were clean. Full stage and runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-fp-check-edges-kqwojnk2`.

## 2026-09-10: full ranking refresh and exact FPCSE reference

At `4db5c4d`, a complete default-object target run finishes with the expected
failing gate: FPDEEP is the only non-event case above 1.5x among then-established
targets (1.62–1.80x). Event references remain provisional, several programs
lack targets, and jumptable remains an emission refusal. This is not a
completion audit or a correctness run.

FPCSE's old provisional 1340 target is now replaced by an independently
derived complete 98-unit listing. Every original intermediate is exact at
SINGLE precision: p=48, q=3/4, s_i=195*i/4 with the two additions kept in
source order. Print binary32 487.5 by value using the original B$PER4 entry;
keep S=, DONE and B$CENP. Actual call identities were checked on all three
primary objects. FPCSEX still needs its own runtime-input reference.

The stronger denominator exposes primary PDS/QB/VBDOS ratios of
173/98=1.77x, 190/98=1.94x, 167/98=1.70x. Nothing got slower. Ten focused
target checks pass; the new reference test failed against the old value.
QB's emitted dump already passes 487.5 as immediate words but retains
initializer stores, collapsed-loop control and stack work. Full stages:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-fpcse-reference-rtam_arc`.
Next substantive work: eliminate that residual work with proper memory and
exception-observation proofs, without weakening the target or FP semantics.

## 2026-09-10: remove overwritten allocation shuffles

The final physical-register peephole now removes register/immediate moves
whose complete byte lanes are overwritten before a read in the same block.
It decodes the selected instructions (or an unchanged opaque instruction),
accounts for AL/AH/AX/EAX overlap and implicit register reads, and does not
infer unconditional writes from conditional-write operands. Calls, branches,
unknown effects and block exits retain conservative live state. Memory loads,
relocated immediates and mandatory interface moves are not removed.

FPDEEP's original four-copy sequence included three pairs of `mov ax,si` /
`mov bx,di`. After eliminating reverse identity copies, five of those six
allocation shuffles have no reader:

```asm
; before                     ; after
movsw                        movsw
mov ax,si                    movsw
mov bx,di                    movsw
movsw                        mov bx,di
mov ax,si                    movsw
mov bx,di
movsw
mov ax,si
mov bx,di
movsw
```

The last BX result is conservatively retained. The copy's direction and
memory semantics are unchanged; this needs no new runtime DF/DS contract.
Two other overwritten register definitions disappear as well. FPDEEP ranking
cost: PDS/VBDOS **1829 -> 1815**, QB **1755 unchanged**, against target 1086.
This is not a cycle measurement and does not close the target gap.

88 focused peephole tests pass. Both emitted-code regressions fail with the
new peephole disabled (six shuffles instead of one); byte-read and partial-write
guards pass. Original/optimized FPDEEP output matches and reaches DONE on
PDS and VBDOS after the final change, with clean linker output. QB output
also matched in the preliminary run; its final ranking is unchanged.
Stage dumps and final runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-overwritten-final-26xdf2u0`.

## 2026-09-10: repair the sliced floating-copy regression

The strict-copy/CSE regression constructed a six-operation floating slice
but kept the original eleven-operation provenance sequence. Its failure was
the verifier correctly rejecting that invalid input, not a compiler regression.
The fixture now describes its actual sliced sequence. The assertions remain:
one shared FLOAD and a lowered `fld st(0)`; the production verifier is unchanged.
All 16 copy tests pass. Disabling scalar copy recognition makes the repaired
test fail at its FLOAD-count assertion, confirming it still detects the feature.

A renewed PDS B$PEI4 library check found the same indirect PRINT dispatch
already recorded in `docs/contracts.md`; it supplies no DF/DS proof. Do not
repeat that tool query expecting a different contract. A contextual dispatch
analysis or a separately established runtime ABI is required. This round
changes no generated program bytes or target ratios.

## 2026-09-10: retain explicit copy environment across CFG edges

The copy frontend now propagates proven direction, DS identity and DS/ES
equality across blocks. A join retains only facts shared by every incoming
edge; unknown calls and selector writes keep their conservative kills.
PUSH DS / POP ES recognition still requires local adjacency. No default
direction flag or runtime preservation contract was invented.

Before: splitting explicit CLD/segment setup from FPDEEP's four MOVSWs
left all four opaque. After: four ordered MIR LOAD/STORE pairs; conflicting
direction, changed DS, and unknown-call paths still refuse scalarization.
The agreeing-edge test failed before the fix. Both block orders are tested.
Focused copy/literal checks: 50 pass, one pre-existing failure in
`test_proven_copy_unlocks_strict_floating_cse` (the test extracts a partial
floating sequence but retains its full original sequence metadata). Restoring
the old block-entry state reproduces that failure. The separately documented
QuickBASIC raw loop-proof assertion was excluded from this focused run.

FPDEEP output objects remain byte-identical on PDS, QB and VBDOS: this fixes
the CFG limitation but does not yet establish the runtime-entry DF/DS facts
needed by that program. No speedup claimed and no unchanged DOS run repeated.
Full stage dumps: `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-copy-edges-2i3_2qg1`.
Next: establish narrowly scoped runtime environment facts for the actual
FPDEEP call path, then scalarize its DOUBLE initializer and measure again.

## 2026-09-10: reprioritize from current emitted targets

### Numeric pool facts now survive validated string-runtime effects

FPDEEP MIX's multiplier remains known across PRINT. The frontend validates
every escaped pool object as a near string descriptor plus payload, or as
VBDOS's two-word indirect descriptor through its selector and FSL_CONST
relocations. Missing/overlapping relocations, nonzero encoded addends,
unknown escapes and unavailable payload bytes refuse the exclusion. Only
complete numeric literal byte ranges outside those objects are excluded from
established OWN runtime write effects. Ordinary writes and unknown calls keep
their normal alias effects; this does not make BC_CN globally immutable.

Exact conversion removal now handles a multiply/conversion chain rather than
requiring an adjacent load/conversion pair. Unused exact producers disappear
through the existing dead-value walk; FCHECK observation points remain.

Before each MIX print (representative PDS sequence):

```asm
fld dword [q]
fmul dword [literal1024]
fistp dword [temporary]
wait
mov eax,[temporary]
push eax
```

After: `wait; push dword 512` (then 768 and 896 for the other iterations).
Measured ranking costs fall from **2123/2217/2123 to 1829/1755/1829** for
PDS/QB/VBDOS, or **1.68x/1.62x/1.68x** against the unchanged 1086 reference.
Original/native FPDEEP outputs match on all three runtimes. FPCSE was also
run on all three after the memory-proof change and prints 487.5 on both sides.
The opaque final DOUBLE initializer remains a blocker to the target.

Focused checks: 65 literal/floating-fact cases and 55 literal/floatfold/alias
cases passed in separate scoped runs. The pre-existing raw
`test_quickbasic_literal_initializers_prove_the_same_floating_exit` assertion
still fails (zero loop-exit proofs); it also fails with HEAD's old initializer
substituted back, and is not weakened or marked as passing. Runtime FPCSE
continues to match. This diagnostic assertion still needs reconciliation.

After `b241ad5`, ran `tools/opportunity.py --targets` against the default
OMF corpus once. Ordinary HARR is **2094–2126 / 1834 = 1.14–1.16x**;
further HUGELP tuning is useful but is not the next reason the documented
target gate fails. Ordinary FPDEEP is **2123–2267 / 1086 = 1.95–2.09x**.
The gate remains incomplete: event references are provisional, some fixtures
have no target, and `jumptable.obj` is unmeasured because emission refuses.
This is a prioritization checkpoint, not a completion claim or cycle timing.

FPDEEP's adjacent MIR/assembly dumps identify two remaining causes:

- Each unrolled MIX expression still multiplies by the literal 1024 and
  converts to LONG at runtime. The SINGLE operand is already known exactly
  as 1/2, 3/4 or 7/8. Its multiplier's BC_CN entry fact is lost across PRINT:
  the pool also contains escaped string descriptors, and the current call
  memory model cannot distinguish their mutable fields from literal bytes.
- The final DOUBLE initializer is four opaque MOVSW operations. The existing
  frontend copy recognizer requires established direction and data-selector
  state locally; neither survives to this post-loop block in its analysis.
  The opaque writes then discard the facts needed to fold DSQ and DRATIO.

Next implementation should improve those proofs at the raise/runtime-contract
boundary. Do not mark all BC_CN memory immutable: string compaction really
updates descriptors. Do not assume DF is clear merely because BC emitted a
forward copy: establish the applicable entry/call contract or retain a safe
representation. Keep runtime effects and aliasing explicit, and leave machine
recognition out of the optimization passes. The reference remains 1086.

## Complete PRESSX and LNGMXX integer references

PRESSX now uses its own **508** target (256 input + 150 arithmetic/final
stores + 102 output), replacing the invalid constant-input 308 denominator.
LNGMXX uses **208** (32 input + 64 arithmetic/final stores + 112 output),
replacing its inherited 210. Full annotated listings and independent BC
block totals are in docs/targets.md; neither target is a fraction of our code.

LNGMXX's reference follows LLVM's signed reciprocal division algorithm,
inspected via git at llvm-project 338e0c9. Clang 21's i386 output independently
confirmed the magic constant/corrections and quotient-plus-remainder identity.
It does not retain our IDIV. Signed boundary checks and wrapped recurrence
checks accompany both references. Final counter and result stores remain.

Before: both ratios suppressed as provisional. After: PRESSX costs
627/631/637 give **1.23x/1.24x/1.25x**; LNGMXX costs 248/250/246 give
**1.19x/1.20x/1.18x**. Assembly and measured costs are unchanged.
29 focused scoreboard tests pass, including both fail-first target checks.
No runtime rerun for this reference-only change. Floating references and
event references remain provisional, missing targets remain missing, and
the whole project is not declared complete.

## HOTLPX has a complete independent reference

Replaced HOTLPX's inherited 312 target with the hand-derived full **217**
listing in docs/targets.md: input 64, arithmetic/final stores 51, output 102.
It keeps runtime inputs unknown and proves the twenty-step wrapped recurrence
equals `20*n*k + 210` modulo 65536, retaining final `i=21` and `s` stores.
The READ/PRINT calls and their stack accesses are included, not treated as free.
Original PDS code independently totals 104 + 58*20 + 10*20 + 102 = 1566.

Before: HOTLPX denominator 312, ratio suppressed as provisional. After:
denominator 217, normal PDS/QB/VBDOS **251/255/261 -> 1.16x/1.18x/1.20x**.
Assembly and costs are unchanged. Event builds remain provisional; other
provisional/missing targets remain blockers to overall completion.
24 focused scoreboard tests pass, including a fail-first target-accounting
regression and wrapping checks. No runtime rerun: this changes only reference
accounting, with existing runtime validation of emitted code left intact.

## Select scaled LEA after allocation

The backend peephole folds an allocated copy/shift/add into scaled LEA when
an immediately following nonzero shift replaces its arithmetic flags. The
removed operations must be inserted instructions owning no original bytes;
relocations, unknown effects, unequal widths, overlapping registers, ESP
indices and mismatched operands refuse the pattern. For word results the
low sixteen address bits equal the original modular arithmetic independently
of the source register's upper half. LEA reads no memory.

HOTLPX before: `mov bx,cx / shl bx,2 / add bx,cx / shl bx,2`.
After: `lea bx,[ecx+ecx*4] / shl bx,2`.
PDS/QB/VBDOS costs **256/260/266 -> 251/255/261**, objects shrink three bytes.
LNGMXX costs **253/255/251 -> 248/250/246**, objects shrink five bytes.
PRESSX costs **632/636/642 -> 627/631/637**, objects shrink three bytes.
This finishes the scaled-address opportunity found while deriving HOTLPX's
reference; the complete target derivation remains provisional.

154-object comparison changes exactly these nine objects with no new refusals.
54 focused host checks and all nine affected runtime cases pass. The three
real HOTLPX LEA regressions failed before implementation. All-stage dumps:
`/tmp/qbopt-lea-before` and `/tmp/qbopt-lea-after`. Runtime evidence:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-lea-gt8xh49d`.

## Lower two-bit constant products to shifts and an addition

The HOTLPX reference investigation exposed its remaining multiply by twenty.
Lowering now expands same-width low integer products whose positive constant
has two set bits: `x*(2^a+2^b) = ((x<<(a-b))+x)<<b`, modulo the original
width. Fresh abstract temporaries expose all lifetimes to allocation. Live
multiply flags, wide/high results, memory operands and partial merges keep
the original multiply. A scaled-LEA form remains a future lowering improvement;
no machine operand was added to MIR and no target denominator was changed.

HOTLPX before: `imul bx,20`. After (input now allocated to CX):
`mov bx,cx / shl bx,2 / add bx,cx / shl bx,2`.
PDS/QB/VBDOS costs **268/272/278 -> 256/260/266**, objects grow seven bytes.
LNGMXX costs **265/267/263 -> 253/255/251**, objects grow nine bytes.
PRESSX costs **644/648/654 -> 632/636/642**, objects grow seven bytes.
DIVMOD's modeled nested-loop cost drops 15000 on each compiler, with five
extra bytes. These are model rankings, not measured hardware speedups.

154-object audit: exactly these twelve objects change, no new refusals.
Twelve focused checks pass (three real fail-first HOTLPX cases, six wrapping
checks, two flag-preservation checks and the existing multiply selection
check). All 69 runtime cases for affected programs pass across three compilers.
All-stage dumps: `/tmp/qbopt-scaled-before` and `/tmp/qbopt-scaled-after`.
Runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-scaled-8tkh95ni`.
The complete runtime-input target derivations remain unfinished/provisional.

## Combine constant offsets in MIR

Algebraic simplification composes single-use ADD/SUB constant chains at the
same modular integer width. Both operations must have no observable auxiliary
results, memory effects, barriers or partial merges. No floating reassociation
or register knowledge is involved; ordinary dead-code elimination removes the
orphaned intermediate. Constants wrap at the original integer width.

SPILL before: `add cx,150 / add cx,70`. After: `add cx,220`.
PDS/QB/VBDOS costs **450/456/460 -> 430/436/440**, objects
**876/859/1116 -> 873/856/1113**. HOTLPX also loses one addition:
costs **270/274/280 -> 268/272/278**, each object three bytes smaller.
HOTLPX's target remains provisional, not certified by this improvement.

154-object audit changes exactly these six objects, with no new refusals.
96 focused algebraic checks pass, including live-flag, shared-result, width,
merge, memory and wrapping cases; the three real SPILL regressions failed
before implementation. Nine strict-LIR runtime cases pass across three
compilers. All-stage dumps: `/tmp/qbopt-offsets-before` and
`/tmp/qbopt-offsets-after`; the MIR already contains the combined constant
before lowering. Runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-offsets-o2hoo5km`.

## Remove jumps to the next emitted instruction

Branch relaxation now removes unconditional direct jumps whose destination
is exactly their emitted end. It iterates with short-branch selection, retains
the address mapping for collapsed labels, and leaves jumps over carried data,
self-loops and relocated operands intact. This is layout, not a MIR pass.

SPILL previously emitted `add cx,70 / jmp next / next: mov [i],si`; it now
falls straight through to the counter store. Its PDS/QB/VBDOS modeled costs fall
**470/476/480 -> 450/456/460**, with two bytes removed per object. Other
changed programs: HOTLOP, HOTLPX, LNGMXX, PRESS, PRESSX, ROTATE and SPLIT.
PRESSX PDS falls 646 -> 644; SPLIT PDS 152 -> 144. These references still
include provisional targets where already documented; no completion claim.

154-object audit: 24 changed objects, no new refusals. All 27 runtime cases
for the eight changed programs pass across three compilers. Five new focused
checks pass, including a real PRESSX symptom that failed before the fix.
Three existing targeted relocation/coverage checks pass. One additional old
BOOLS fold-test fails because it finds no qualifying fold; verified unchanged
with the previous assembler, and not weakened or included in the pass count.
Before/after all-stage dumps: `/tmp/qbopt-fallthrough-before` and
`/tmp/qbopt-fallthrough-after`. Runtime evidence:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-fallthrough-j_82rc5r`.

## Short zeroing when arithmetic flags are dead

The post-allocation peephole replaces a word/dword immediate zero with a
same-width XOR only when later work in the same block overwrites all arithmetic
flags before observation. Calls, unknown instructions, partial flag writers,
shifts and block boundaries stop the proof. Relocated zero operands remain
addresses, and byte moves are left alone because XOR saves no bytes there.

HARR: `mov cx,0` -> `xor cx,cx`; the later descriptor-offset ADD replaces the
flags. Object sizes PDS/QB/VBDOS: **852/843/1036 -> 851/842/1035**. Modeled
costs **2246/2262/2256 remain unchanged**. This is a one-byte code-generation
improvement, not a loop-speed gain or closure of the larger target gaps.

154-object comparison changes only those three HARR objects with no new
refusals. 42 focused host checks and three strict-LIR HARR runtime comparisons
pass. Disabling the change makes the real emitted-code regression fail.
All-stage dumps: `/tmp/qbopt-zeroing-before`, `/tmp/qbopt-zeroing-after`;
the adjacent prologue/peephole diff shows only the CX zeroing replacement.
Runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-zeroing-uwzzcdfn`.

## Assembly dumps expose relocation targets

Stage assembly previously displayed every relocated operand as zero, hiding
the distinction between array cells and runtime callees. It now annotates
each fixup with its instruction-relative location, kind, target and displacement.
The original bytes remain visible; the annotation describes linker work rather
than pretending the object is already linked.

This immediately corrected an event investigation assumption: BOOLS PDS's
stub at 0x003d jumps to **B$EVK1**, not B$EVCK. The original FIXUPP at 0x003e
names that external. Do not substitute the EVCK contract for this dispatcher
or waive the unknown near-call interface at 0x0048.

Before: `jmp 0:0`. After: `jmp 0:0 ; reloc +0x1: ptr16:16 B$EVK1+0x0`.
Generated assembly bytes and modeled costs are unchanged. Seven focused stage
tests pass; both added display regressions failed before the change. This is
diagnostic progress, not a reduction in the remaining optimization gap.

## Proven allocation identity removes the element reload

The bounded array proof establishes both extent and the descriptor-derived
segment for every marked access. Equal offset SSA values in that same proven
allocation therefore identify equal bytes without segment-register SSA.
Memory equality now uses that fact, while missing or different allocation
proofs still refuse equality. Existing forwarding removes the immediate reload
of HARR/SEGLD's just-stored element.

PDS costs: **HARR 5062 -> 4462 (2.43x)**;
**SEGLD 13456 -> 11056 (1.65x)**. Two new regressions fail against old memory
equality, including HARR's surviving element load; 20 focused checks and six
strict-LIR runtime comparisons pass across p-g2/q-O/v-g3. Segment setup remains
in the loop and remains the next placement opportunity.
Dumps: `/tmp/qbopt-allocation-equality`; runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-allocation-equality-hkp11p3v`.

## Induct the complete offset pointer, including invariant descriptor data

Affine composition now accepts a proven unchanged direct cell as an additive
offset. Strength reduction reads it in the preheader and advances the full
offset pointer thereafter. Symbolic descriptor references are canonicalized for
the inserted operand; no register names enter the induction analysis.
HARR's adjusted-offset read leaves the inner loop, which advances by 42 bytes.
The segment reload still remains in the loop.

PDS modeled cost: **HARR 7130 -> 5062, 3.89x -> 2.76x**;
**SEGLD 16896 -> 13456, 2.52x -> 2.01x**. Forty focused induction checks
pass; the real HARR preheader-read regression fails with old analysis, and a
descriptor-write variant keeps the read inside. Nine strict-LIR runtime
comparisons pass (HARR/SEGLD/NESTED across p-g2/q-O/v-g3).
Dumps: `/tmp/qbopt-invariant-pointer`; runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-invariant-pointer-p8ksw05l`.

## Reuse unchanged memory-dependent computations

CSE now admits explicit cell operands and checks intervening writes/barriers
before reusing them within one block. Far references without segment identity
remain excluded. This removes HARR's duplicate descriptor-adjusted address;
the existing induction-variable reduction still supplies the 42-byte step.
PDS costs: HARR **7930 -> 7130 (3.89x)**, SEGLD **20096 -> 16896 (2.52x)**.
The larger remaining goal is invariant descriptor/base/segment placement and
complete-pointer induction, not merely duplicate-expression removal.

Three fixture regressions fail against old CSE; corresponding unknown-write
cases retain both computations. Fourteen focused checks and nine strict-LIR
runtime comparisons pass (HARR/SEGLD/LNGMIX across p-g2/q-O/v-g3).
Dumps: `/tmp/qbopt-memory-cse`; runtime evidence:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-memory-cse-hslacsvl`.

## Bounded array paths unlock existing optimization

The raise now proves finite word-width array paths by exact scalar evaluation.
Each far access must use the descriptor's loaded segment and adjusted pointer,
and fit the allocation before its memory effect is interpreted. Unknown branches,
calls before later accesses, descriptor writes, unsupported operations, nonzero
lower bounds and traversal exhaustion discard the proof. No alias assumption is
used to establish the bounds. The resulting allocation metadata only excludes
direct accesses to the descriptor's program-data segment; other pointers remain
conservative. This is a bounded proof, not general symbolic range analysis.

Existing constant propagation and promotion now retain descriptor dimensions and
counter values across the element stores. PDS modeled costs improve:
**HARR 11294 -> 7930, 6.16x -> 4.32x**;
**SEGLD 25002 -> 20096, 3.73x -> 3.00x**.
Both still miss the 1.5x target. Segment-load optimization and further loop/codegen
work remain. No array-specific optimization pass was introduced.

Three real HARR dimension-fact regressions fail with the proof disabled; 35
focused checks pass, including rejected out-of-bounds/unknown-condition/changed-
descriptor/wrong-segment/exhausted proofs. Six strict-LIR runtime comparisons
pass for HARR/SEGLD across p-g2/q-O/v-g3. Stage dumps display allocation proofs:
`/tmp/qbopt-array-bounds-final`. Runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-array-bounds-k2_79k98`.

## Memory identity requires the segment, not just the offset

HARR's descriptor-segment loads still produce Opaque results; element references
have no segment SSA identity. `same_bytes` previously accepted two such far
references as equal, even though an intervening segment reload can change the
address. It now requires known segment identity for far-reference equality.
Dead-store coverage reuses that equality rule after aligning displacements;
its old independent check also ignored relocated segment indices, allowing
equal offsets in different objects to appear to cover each other.

Three failing-before cases cover unknown segment, changed segment and different
relocated objects; a positive case preserves coverage for known same pointers.
3417 availability checks pass in 15 seconds; nine strict-LIR runtime comparisons
pass (HARR/SEGLD/NESTED across p-g2/q-O/v-g3). HARR's emitted stage is unchanged.
Dumps: `/tmp/qbopt-far-identity`; runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-far-identity-7nomd5wb`.

Next array proof must include segment provenance as well as offset bounds;
neither an unchanged offset nor a DIM request alone proves address identity.

## Numeric DIM postconditions reach constant propagation

The raise now emits generic normal-return memory values for numeric DDIM:
dimension count, element width, and each dimension's count/lower bound.
The descriptor layout stays in raising_arrays; constant propagation sees only
memory/value pairs, and stage dumps print them. Unknown stores still kill
the facts. This does not prove bounds or disjointness for element accesses.

Verified against shipped BCOM45/BCL71ENR/VBDCL10E dynamic.asm disassembly:
dimension count at +8, element width +12, counts/lower bounds +14/+16 with
stride 4. The stack is consumed backwards, so the last dimension is first.
QB's stores are at 0034/003a/004b/0052; PDS/VB at 0017/001d/0030/0037.
Only recognized compiler families and numeric allocation attributes use these
facts. Unknown families, strings, wrapping descriptors, RDIM and locally defined
runtime-name substitutes do not acquire these postconditions.

All three real HARR postcondition regressions fail with the old constant walk;
33 focused checks pass. Six HARR/SEGLD emission comparisons across the compiler
families are byte-identical with/without postconditions, so no new runtime loop
was needed. No speedup yet: loop element stores still conservatively invalidate
the facts. Dumps: `/tmp/qbopt-dim-postconditions-visible`.

## Constant memory facts retain pointer identity

The constant-cell walk recorded a constant pointer-relative store using only
its displacement, dropping the base/segment SSA value. A direct read could
then incorrectly acquire that constant. It now invalidates potentially aliased
facts but records a new direct fact only for a proven direct address. Proven
symbolic references are canonicalized on both reads and writes, making the
descriptor-address metadata usable by constant propagation.

Three new cases fail before the fix; 1030 focused constant/array checks pass.
Nine strict-LIR runtime comparisons pass (HARR/SEGLD/LNGMIX across p-g2/q-O/v-g3).
HARR's final MIR is identical to the previous stage dump; no speedup claimed.
Dumps: `/tmp/qbopt-constant-pointer-identity`; runtime evidence:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-constant-pointer-68wnorjb`.

Runtime source inspection also confirms that dynamic allocation can be near:
QB45 `dynamic.asm` uses the near allocator and DGROUP when FADF_FAR/HUGE are
clear. Array provenance therefore still requires an in-bounds proof; a DIM
request alone cannot establish disjointness from program data.

## Proven descriptor addresses without changing relocation operands

The raise now attaches a symbolic effective address to descriptor references
whose pointer is a known word-width symbol. Memory equality and alias checks
use that address; lowering retains the original address/base and relocation
operands. Thus HARR's descriptor fields at offsets 8 and 16 can be identified
without turning their unrelocated pointer-relative operands into invented
relocations. This proves field identity, not field contents or heap bounds.

Far accesses, wide pointers and effective offsets that wrap remain unknown.
Nineteen focused checks pass; all three compiler-family metadata regressions
fail against the old raise. HARR and SEGLD emit byte-identical objects with
and without the metadata on p-g2/q-O/v-g3 (six comparisons), so no additional
runtime loop was needed. Costs are unchanged. Array-element provenance and
in-bounds reasoning remain the next required part of the work.

## HARR: the next major gap is array provenance, not another arithmetic pass

Current stage evidence (`/tmp/qbopt-harr-current`, especially s43-mir-widen):
the raise recognizes DDIM's `(0..20, 0..20)` bounds and element width 2,
but that request is not connected to later element references. Those remain
`[es:bx]` with no allocation identity. Descriptor accesses likewise remain
`[abs+si+0xa]` and `[abs+si+0x2]`, even though their base comes from the
descriptor symbol. `raising_arrays.annotated` annotates allocation calls only.

Consequently a store through the element reference may alias the counters
and descriptor, so promotion, CSE and LICM correctly retain their loads.
The final loop still loads the dimension word at 0x5e, multiplies at 0x61,
recomputes the descriptor-based address at 0x72 and 0x7d, and reloads the
stored element at 0x83. The loop counter reload at 0x8a also blocks a simple
SSA recurrence proof. Adding another strength-reduction pattern cannot
resolve these missing memory facts.

Next implementation milestone: connect the versioned runtime allocation
contract to descriptor and element provenance in the raise, with an explicit
in-bounds proof before claiming disjointness from program data. Do not treat
every far access as heap memory or infer safety merely from a DIM request;
unknown indexing and descriptor mutation must remain conservative. Preserve
the original relocation-bearing operands in lowering while carrying semantic
provenance separately. This is needed to unlock the existing general passes,
not to add machine-aware special cases to them. No HARR speedup is claimed.

## Allocate sign extraction without fixed AX/DX when flags are dead

ADDRM's promoted accumulator added pressure around CWD's fixed-register
interface, producing a spill. Lowering now selects copy plus arithmetic
shift for word/dword sign extraction when flags are dead; the operands stay
abstract until allocation. Otherwise it retains the original flag-preserving
conversion. The decision uses backward flag liveness after branch scheduling,
including block live-outs, not a scan of the next instruction alone.

ADDRM cost **1794 -> 1320 (2.38x -> 1.75x)**, improving on the earlier 1492
baseline as well. The new dead-flags test fails before the change, and the
live-flags cross-block case retains CWD. Nine focused lowering checks and
nine strict-LIR runtime comparisons pass (ADDRM/LNGMIX/NEGNOT across the three
compiler configurations). Dumps: `/tmp/qbopt-sign-shift`; runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-sign-lower-2h24eknr`.

## Preserve independent promotion when an update cannot be split

The PDS scoreboard refresh caught SEGLD regressing 25002 -> 28202: an
unpromotable memory update caused promotion to return the original body,
discarding unrelated counter promotions too. Retry write-through promotion
without update splitting in that case; retain the unsafe update in memory.
SEGLD returns to **25002/6704 = 3.73x**. A real-fixture regression fails
before the fix. Twelve focused promotion checks and nine strict-LIR runtime
comparisons pass (SEGLD/NESTED/HARR across p-g2/q-O/v-g3). The older
promotion-store test now isolates store motion, whose separate tests verify
the store's new exit placement rather than its original instruction address.

Dumps: `/tmp/qbopt-segld-promotion-fixed`. Runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-promote-independent-7qcme0hf`.

Current PDS measurements also show ARRIDX 0.89x, PRESS 0.82x, SPILL 1.37x,
SPLIT 1.30x; MATRIX 1.62x, ROTATE 1.54x and IVCHAN 1.51x remain above goal.
ADDRM regressed to 2.38x from the earlier 1.98x and is not fixed by this
change. HARR 6.16x and floating-point cases above 3x remain major gaps.

## Affine addresses with invariant offsets

Induction analysis now retains invariant additive terms while composing
word-width arithmetic. Strength reduction initializes these terms outside
the loop and advances only the scaled counter. This recognizes NESTED's
`(rowBase + column) * 2`: its address now advances by two instead of adding
row and column then shifting on every inner iteration. No machine register
names enter the analysis or transform.

NESTED cost **1962 -> 1900, 2.55x -> 2.47x**. The real-fixture regression
fails with the old implementation; nine runtime comparisons pass across
NESTED/MATRIX/ADDRM on p-g2/q-O/v-g3. Stage dumps:
`/tmp/qbopt-affine-offset`. Runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-affine-offset-hx0x5oew`.

## Store motion through a nested loop body

A rotated loop need not have only two blocks: a unique latch with a direct
backedge still runs on every completed iteration. Store motion now uses
that property while retaining the unique header exit, zero-trip seed and
whole-loop observability checks. NESTED's accumulated sum is stored once
after the outer loop instead of once per row; initialization remains.
Cost **1990 -> 1962, 2.59x -> 2.55x**. It still misses the overall goal.

The real-fixture regression failed before this change. All 13 store-motion
checks pass, as do nine strict-LIR runtime comparisons of NESTED, MATRIX,
HOTLOP across p-g2/q-O/v-g3. Dumps: `/tmp/qbopt-outer-store-sink`.
Runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-outer-sink-o3uxyvdc`.

## Sink the nested accumulator's inner-loop store

The zero-trip seed proof now follows predecessor edges and translates phi
inputs, instead of requiring initialization in the immediate preheader.
Every incoming path must establish the same cell/value pair; an unknown
write, barrier or uninitialized entry rejects the move. Backedges discharge
the same inductive obligation, while entry paths still need an actual store.
This recognizes NESTED's outer phi as the inner loop's memory seed.

NESTED now writes its sum at the inner-loop exit (five times instead of 30),
not on each inner iteration. Cost **2172 -> 1990; 2.83x -> 2.59x**. The
outer-loop exit store and remaining allocation costs are still opportunities.
The new real-fixture regression fails before the change; removing its seed
keeps the store in the loop. All 12 store-motion checks and nine strict-LIR
runtime comparisons pass (NESTED/HOTLOP/LNGMIX, p-g2/q-O/v-g3).
Dumps: `/tmp/qbopt-nested-store-sink`. Runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-nested-sink-9g9z4h2i`.

## Promote read-modify-write accumulators

Promotion previously excluded arithmetic with both a memory read and write.
It now separates eligible fixed-cell updates into a value computation and an
observable store, then uses the existing availability and SSA construction.
If a separated read cannot be served from a proven available value, the
original body is retained. NESTED's sum now flows through the nested loop
phis. Its write remains: this enables subsequent store-motion work, not a
speedup by itself (2156 -> 2172, **2.81x -> 2.83x**).

The first runtime trial printed T=0 instead of 675: lowering discarded the
inserted store's relocation ownership. STORE now selects its own machine
operation, and lowering carries its symbol marker into LIR. The computation
owns the original bytes; the inserted store owns the relocated operand.
Both the promotion and backend ownership regressions failed with their old
implementations. Fourteen focused checks pass; NESTED/HOTLOP/FLAGS pass on
p-g2, q-O and v-g3 (nine runtime comparisons).
Dumps: `/tmp/qbopt-rmw-promote-verified`. Runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-rmw-final-rto51u8w`.

## One reload per spilled operand

The NESTED allocation dump contained two identical frame reloads before each
outer-loop multiply. The LIR instruction names the same value twice; the
spiller made two fresh reloads, overwrote the first rename, and used only the
second. Reusing the first rename removes the unused reload without changing
operand multiplicity or any MIR pass. NESTED cost falls 2228 -> 2156,
**2.90x -> 2.81x**. Adjacent backend dumps show exactly two removed loads:
`/tmp/qbopt-single-spill-reload`.

The repeated-operand regression failed before the change; all 15 spill tests
pass. NESTED and SPILL pass strict-LIR runtime checks on p-g2, q-O and v-g3
(six comparisons), artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-single-reload-k5nmekdv`.

## Frame-to-frame phi copies

NESTED-p-g2 refused LIR at 0x004b because both ends of a parallel copy
were spilled. The spiller now keeps that copy grouped with both frame slots
explicit. After dependency ordering, parcopy expands it to a balanced memory
PUSH/POP pair, preserving flags without another scratch register. Selection
now supports 16/32-bit memory POP. Cyclic copies remain explicitly refused.

NESTED now emits LIR at 2228/768 = **2.90x**, not yet the 1.5x goal.
Stage dumps: `/tmp/qbopt-memory-parcopy-final`. Seven focused checks pass;
the spill regression failed against the old spiller, and both encoded-width
checks failed against the old selector. Nine saved runtime results pass:
NESTED, PRESSX, SPILL across p-g2/q-O/v-g3, checked against golden output and
DONE markers, not merely matching two empty files. Artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-mem-parcopy-c2493ga4`.
The earlier wider focused run had two unrelated MIR-versus-REFUSED expectation
failures; this is not a claim that the full test suite is green.

## Constant-divmod experiment — withdrawn

`db942bd` exposes LNGMIX's 100000 dividend. Replacing its constant DIVMOD
with quotient/remainder copies (14285 and 5) was prototyped but not retained.
Production remains unchanged: the prototype refused emission with
`0x005f: 12 bytes between the ops are not instructions`.

More importantly, adjacent MIR dumps pinpoint an SSA defect in round-three
hoist: the accumulator phi `v11_1 := phi entry:v11_2, latch:v1_6` disappears,
and its loop use becomes a use of the entry zero. Round-four fold then correctly
folds that *incorrect input* to `0 + 14290`. Fix hoist's handling of existing
cross-variable phis before reintroducing the divmod fold; do not blame the
constant evaluator for the phi already lost in the previous stage.

Evidence: `/tmp/qbopt-lngmix-divfold-20260909`, especially
`s29-mir-r03-fold.txt` versus `s32-mir-r03-hoist.txt`, then
`s42-mir-r03-place.txt` versus `s43-mir-r04-fold.txt`.
The prototype copied both semantic results, retained original byte ownership on
the first copy and used zero-width ownership on the second. Ownership remains
another blocker. No speedup or runtime success is claimed for this experiment.

## VBDOS procedure-exit interface

PROCS `/G3` now emits through LIR rather than refusing B$EXSA at 0x113.
The exit retains all six allocatable general-register inputs, including DX:AX
exported to the BASIC caller. Disassembly of VBDCL10E.LIB rtenexit.asm at 0x68
establishes that the incoming arithmetic flags are overwritten; the normal
frame-restoring return removes no caller arguments. Helper effects remain
unknown: no memory, clobber, or control guarantee is relaxed. Full-library
contract analysis is evidence for unresolved dependencies, not an automatic ABI.

The emission regression failed first; five focused checks pass. Strict-LIR
runtime checks pass PROCS on PDS `/G2`, QB `/O`, and VBDOS `/G3` (three cases
each). Stages: `/tmp/qbopt-procs-exit-20260909`. This removes a coverage blocker,
not a claim of target completion or a full integration gate.

Goal: correct modern-compiler-quality output, machine-independent MIR, every documented target within 1.5x. **Not complete.** Branch `restore-through-lir`; checkpoint `dda3d46` and backend increment `abf9ab5` are committed. Stashes are untouched.

## Operand-width increment

Constant reads now mask and narrow a fact to the semantic operand's width. Previously a word read of 0x12350000 shifted right once folded to 0x8000 instead of 0. Three byte/word cases failed first, passed after the fix, failed with the mutation restored, then passed again. The constant suite had 985 passes and one existing comparison-materialization failure, reproduced with old read semantics. Ruff passes for both changed files. Sampled hotlop, lngmix and harr costs are unchanged; this is correctness progress, not a measured optimization gain.

The next increment teaches constant propagation MIR's existing step semantics, restoring increment/decrement folding without machine names. Six width/wrap cases and the existing cmpord regression fail when this change is removed; the complete constant-propagation file now passes 992 tests (15.75 seconds). PDS bools improves from 176 to 174 (1.38x); hotlop/lngmix are unchanged. Production stage files: `/tmp/qbopt-constant-steps-20260909`. No all-target or runtime-completion claim.

Committed as `c72f918`. The following alias increment requires equal segment origins as well as equal offset values before using displacement arithmetic. Distinct or unknown far segments can overlap despite disjoint offsets (1000:0020 equals 1001:0010). Three new checks fail first and under mutation; the alias/loop-motion checks pass 12 tests. Sampled harr, matrix and segld costs remain unchanged. This repairs a soundness prerequisite, not the remaining array-extent proof.

Alias fix committed as `ffb0269`. A strength-reduction experiment appeared to improve harr from 6.27x to 4.47x and passed harr's output check on all three compiler families. **Do not accept that as valid optimization evidence**: the dumps show the shifted full address being mistaken for both loop counters because their old variable numbers match. harr writes and immediately rereads an address, so its sum does not independently validate that address. Artifacts: `/tmp/qbopt-strength-harr.zYw8Ia`. Strength remains disabled. The next analysis fix keys recurrences and backedges by exact SSA identity; two focused checks fail first and under mutation, then pass. Fresh variable collisions in strength.py and proper SSA construction remain to fix before enabling it.

Exact identity committed as `01ebaaa`. The old raw-MIR matrix coverage assertion now fails (zero recognized counters); it relied on equality not proven by its input. The next increment follows only width-preserving value copies, finding three exact counters/two derived candidates in promoted matrix and one/one in segld. Raw-memory equivalence still requires promotion, not variable-name matching. Generic isolated SSA construction is extracted from promotion into `ssa.py` and used by strength reduction. New counters now use fresh variable IDs from the complete graph and carry an explicit loop phi; removing either fix fails the regression. An unavailable stride is refused, and duplicate transformations are skipped. Strength remains disabled: no verified performance gain from the repaired implementation yet, and multiple-entry/backedge recurrence validation and profitable selection remain outstanding.

## Current implementation

The checkpoint introduced write-through memory promotion, allocation/spill corrections, MIR fixed-point iteration, truthful measurements and dumps of actual production passes. No re-raising of emitted bytes.

Since the checkpoint:
- Sink unobserved fixed-cell loop stores into a dedicated single exit. The store must execute in the exiting block; observers and unknown effects prevent motion.
- Coalesce equal copies using def/live-out interference, preserving unequal live-in values and partial-width distinctions. Normalize register classes and rename nested memory operands.
- Preserve block labels when their first instruction disappears, and transfer removed leading-copy byte ownership across inserted zero-span copies.
- Allocate constraint IDs above retained pins, fixing QuickBASIC/PDS division refusals.
- Remove the production MIR-emitter fallback. Backend refusal returns the byte-identical input and original diagnostic, never another emitter's output. Historical emission enum/field remain for compatibility.

## Measurements

Selected PDS `/G2` modeled costs, not hardware timings:

| Program | Checkpoint | Current | Target | Current ratio |
| --- | ---: | ---: | ---: | ---: |
| press | 420 | 326 | 308 | 1.06x |
| hotlop | 646 | 452 | 312 | 1.45x |
| lngmix | 921 | 867 | 210 | 4.13x |
| harr | 11694 | 11494 | 1834 | 6.27x |
| matrix | 14708 | 12076 | 6210 | 1.94x |
| segld | 26602 | 25802 | 6704 | 3.85x |

Checkpoint bools was 176/126 (1.40x); not remeasured in the latest selected run. No final all-target claim. The full opportunity command completed but its output was lost to truncation, so it supplies no recorded evidence.

The last full emission scan was 410 LIR / 77 MIR fallback before fixing seven jumps refusals and removing the fallback. Do not treat that as the current count.

## Validation

- Changed backend modules: 1,519 passed, two failures in historical fallback expectations (71.65 seconds).
- Those failures exposed a second emitter hiding injected backend diagnostics. Updated tests require refusal, byte-identical input, and the original reason; three failed before removing fallback. All five selected refusal checks now pass.
- Final focused check: 43 passed in 1.09 seconds. Rechecked hotlop 1.45x, press 1.06x and matrix 1.94x after fallback removal; matrix still fails the target. Ruff passed for the new loop-motion/coalescer work and edited whole-segment emitter; this is not a whole-project lint claim.
- Focused runtime programs passed on PDS, QuickBASIC and VBDOS: hotlop, hotlpx, press, pressx, matrix, harr, lngmix, flags. After address-class changes: jumps, arridx, arrprm, fpdeep, procs, harr, hotlop and pressx passed all three. VBDOS procs used unchanged input, not LIR success.
- Latest runtime artifacts: `/tmp/qbopt-coalesced-addresses-runtime-20260909`. Stage evidence: `/tmp/qbopt-lngmix-next-20260909`, `/tmp/qbopt-harr-next-20260909`, `/tmp/qbopt-jumps-coalesce-20260909`.
- Full commit gates have not passed. The checkpoint had lint failures and 787 type diagnostics; pytest was interrupted once its gate was already blocked. User authorized that unsigned checkpoint with hooks bypassed, not future commits.
- Subsequent authorization: use focused checks for progress commits and reserve the full gate for milestones. Current full type check reports 802 diagnostics; the milestone gate remains outstanding. Progress commits may bypass hooks under this explicit authorization, without claiming full validation.
- Required independent review was unavailable (expired Claude OAuth; Fable unavailable); user authorized proceeding. No Claude Desktop session was resumed.

## Next implementation priorities

VBDOS entry-contract checkpoint: `B$ENRA` can now be lowered at sites with
an unrelocated immediate `MOV BX,0` immediately before the call in the same
basic block. `tools/libdump.py B$ENRA` shows VBDCL10E.LIB's rtenexit.asm:
entry 0x17 builds the frame from CX; `or bx,bx` at 0x4b skips the unresolved
helper call at 0x5b when zero. Both BX and CX remain fixed call inputs,
so the selector's zero value must survive allocation. All other effects use
the worst-case contract. Nonzero, relocated, unknown or separately entered
call sites remain unestablished. Four focused checks pass; removing the
site specialization fails the positive case. PROCS is **still refused**:
the next exposed blocker is VBDOS `B$EXSA` at 0x113, not B$ENRA. No claim
of completed procedure support or runtime success is made for this increment.

ADDRM ownership checkpoint: VBDOS/G3's three-byte emission refusal at 0x80
is fixed. CSE had correctly transferred the deleted index reload's bytes to
the preceding high-word store. Widening recomputed its chain end from the
original instruction nodes, discarding that transferred ownership. Pair and
chain endpoints now honor current `covers`, with original node spans only as
fallback. The real-object emission regression failed before the fix and now
requires strict LIR output. ADDRM, ARITH and NEGNOT pass runtime checks on
PDS/G2, QB/O and VBDOS/G3 (nine program/configuration runs). Before/after
stage dumps: `/tmp/qbopt-addrm-ownership-20260909` and
`/tmp/qbopt-addrm-ownership-fixed-20260909`. The known PROCS/TWICE runtime
contract refusal remains, and broader full-goal validation is outstanding.
Five existing widening ownership/restore checks also pass (75.93 seconds;
these five checks internally traverse the fixture corpus, so they were run
once, not as a repeated test loop).

**Strength reduction is now enabled by default**, restricted to multiplication
chains in innermost loops. ADDRM's cheap shift chains regressed 2,308 -> 2,578;
excluding shift-only formulas keeps it at 2,308 and avoids an unnecessary
counter in ARRIDX as well. This policy deliberately forgoes the earlier
shift-only SEGLD/stride gains until pressure-aware selection exists.
Lowering rejects an inserted ADD/MUL crossing a live condition when comparison
scheduling cannot preserve it, including conditions live across block edges.
This is a correctness guard, not successful optimization of those cases.

Current PDS/G2 modeled costs, strength off -> on:
matrix 11,916 -> 11,418; arridx 1,138 -> 742; split 904 -> 508;
ivchan 1,399 -> 1,023. ADDRM 2,308, SEGLD 25,802 and stride 1,882 are unchanged.
The three-family emission check is 94/96 LIR: all 32 PDS/G2 and 32 QB/O;
30 VBDOS/G3. Its two pre-existing refusals remain ADDRM's unowned bytes at
0x80 and PROCS/TWICE's unestablished B$ENRA interface. Seventeen of eighteen
focused runtime program/configuration runs pass; VBDOS ADDRM explicitly
reports REWRITEFAIL, not PASS. Seventy-four focused host checks pass.
Removing either the cheap-work filter or the condition guard fails its
regressions. Production stages: `/tmp/qbopt-strength-production-arridx-20260909`.
Full milestone validation remains incomplete; the overall goal is not met.

Condition-selection checkpoint: lowering now schedules a pure, single-use
comparison immediately before its terminal branch. A synthesized stride add
between them previously left the branch reading the add's machine flags,
despite MIR naming the comparison's condition value. The change is confined
to lowering, following the adjacency role of LLVM SelectionDAG glue; MIR
passes retain their semantic ordering. Memory/effectful comparisons, additional
results and multiple consumers are not moved. Those general cases still need
condition materialization or flag-aware scheduling before unrestricted use.
The new ordering regression fails with scheduling removed; 40 focused checks
and 12 strict LIR program/configuration runs pass (matrix, segld, bools and
flags across three compiler families). Dumps:
`/tmp/qbopt-conditions-matrix-20260909`. Strength remains disabled and full
milestone validation is outstanding.

Innermost selection checkpoint: reducing matrix's outer row counter created
a live range across its inner loop and a spill/reload on every inner iteration.
Restricting strength reduction to innermost natural loops avoids that loss:
experimental matrix cost is now 11,418 versus production 11,916; segld is
24,642 versus 25,802; HARR is unchanged at 11,094. The new real-output cost
regression fails at 12,974 when the restriction is removed. Thirty-six focused
checks and six strict LIR runtime cases pass. Full dumps and final assembly:
`/tmp/qbopt-inner-strength-matrix-20260909`. Strength is still disabled pending
backend condition/flag safety and a broader milestone validation. This is a
temporary selection policy, not a substitute for target-aware pressure costing.

Affine-address checkpoint: induction analysis composes word multiply,
same-counter add/subtract and shifts, including known SSA constants. Matrix's
diagonal `(i * 20 + i) << 1` is recognized as stride 42; strength reduction
chooses terminal candidates rather than introducing counters for every term.
Thirty-five focused checks pass; disabling composition makes the two new
coefficient/wrap regressions fail. Six strict LIR runtime cases pass for
matrix/segld across PDS /G2, QB /O and VBDOS /G3. Stage-by-stage evidence is
in `/tmp/qbopt-affine-matrix-20260909`.
Strength remains disabled: experimentally enabling it costs matrix 12,974
versus production 11,916, though segld improves 25,802 -> 24,642. This is
analysis infrastructure, not a production performance gain. Profitable
selection and latch flag safety remain prerequisites to enabling it. The
focused lint invocation reports existing annotation/zip diagnostics; no
full-gate claim.

Dead-phi checkpoint: dead-code elimination now removes unused phi cycles,
retaining externally demanded values and all real argument/address readers.
Dependencies propagate through live phis; obsolete preservation-only references
are removed with dead phis. Matrix's unused high-product phis no longer block
projection, and its final assembly contains immediate `imul` forms. PDS modeled
cost falls 12,334 -> 11,916 (1.92x); pressx falls 820 -> 782. HARR remains
11,094. Seventy-two focused checks and 15 strict LIR runtime cases pass
(matrix/pressx/HARR/nots/lngmix across PDS /G2, QB /O, VBDOS /G3). Disabling
phi pruning fails the real-matrix regression. Dumps and emitted assembly:
`/tmp/qbopt-matrix-dead-phis`. Full validation remains outstanding.

Algebraic checkpoint: a machine-independent pass now simplifies integer
identities and projects a two-result word multiply to its low result when
the high answer, flags and preserved upper bits are unobserved. Actual reads
and phi inputs prevent projection; discarded preservation references are
removed with the discarded definitions. This lets lowering choose ordinary
two-address/immediate multiply forms without accumulator-pair constraints.
HARR PDS now emits `imul bx,cx`; modeled cost falls 11,494 -> 11,094 (6.05x,
still far above 1.5x). pressx changes 824 -> 820; the sampled matrix, SEGLD,
lngmix and arridx costs remain unchanged. Forty-six focused/algebraic-boundary
checks pass; disabling projection fails the real-HARR regression. Strict LIR
HARR/matrix/nots/SEGLD/lngmix runtime passes across PDS /G2, QB /O and
VBDOS /G3 (15 cases). `/tmp/qbopt-low-product-harr-20260909` contains every
stage and the emitted assembly. Full validation remains outstanding.

Copy-propagation checkpoint: CSE now substitutes full-width copy values into
their uses and removes the copies. Low-word copies are also eligible when
the existing bit-demand analysis proves their preserved upper word unobserved;
wide readers and partial reads of a wider source still prevent substitution.
HARR's first CSE stage removes copies at 0x5c, 0x6b, 0x6d and 0x7b. Required
machine moves are reintroduced downstream: modeled PDS costs remain unchanged
for HARR, SEGLD, pressx, lngmix and matrix. This improves the MIR boundary,
not the target scoreboard yet. Thirty-five focused checks pass; restoring the
old copy-retention code fails the regression. Strict LIR runtime passes those
five programs on PDS /G2, QB /O and VBDOS /G3 (15 cases). Stage evidence is in
`/tmp/qbopt-copy-values-harr-20260909`.
Rejected experiment: simply allowing CSE to share partial-write copies lowered
HARR 11,494 -> 11,296 but worsened SEGLD 25,802 -> 26,604. That relaxation is
not enabled; direct source-value propagation avoids the measured regression.

Array-request checkpoint: raise recognizes DDIM/RDIM argument setup and attaches
`ArrayRequest` to the call: symbolic descriptor, element width, dimension bounds,
and whether it replaces an existing allocation. Recognition stays in
`raising_arrays.py`, outside MIR passes. The ABI is documented in QB 4.5
`runtime/rt/dynamic.asm`; real HARR and SEGLD objects confirm argument setup on
PDS /G2, QB /O and VBDOS /G3, including QB's register-fed pushes. ADIM is not
classified as allocation. Thirteen focused checks pass; annotation preserves
strict LIR emission byte-for-byte for HARR on those three configurations.
`/tmp/qbopt-array-request-q-20260909` shows the request at raise and after opt.
These are requested shapes, NOT proof of successful allocation, physical
disjointness, lifetime, or in-bounds access. Next use requires those proofs;
do not feed requests directly to no-alias. No target reduction claimed here.

CSE semantic-operands checkpoint: optimizer-created operations no longer need
an original decoded node to participate in value numbering. Symbolic operands
also participate by their complete identity, never by their encoded zero.
The originless and symbolic cases fail under the old computation key; 33
focused checks pass, including distinct-address negatives. Strict LIR
HARR/matrix/chain/segld runtime passes on PDS /G2, QB /O and VBDOS /G3
(12 program/configuration cases). HARR stage dumps are in
`/tmp/qbopt-cse-symbolic-harr-20260909`. No new target reduction is claimed:
partial-write preservation and the far-store/descriptor alias barrier remain.

CSE value-identity checkpoint: removed the obsolete same-variable restriction
on common-expression reuse. The dominator/value/width checks still apply;
register placement belongs to allocation, not this pass. The regression now
exercises both direct and phi uses across different variable identities and
fails with the old restriction restored in memory. All 25 induction/SSA checks
pass, and strict LIR chain/matrix/lngmix/pressx runtime passes on PDS /G2,
QB /O and VBDOS /G3 (12 program/configuration cases). PDS chain shrinks three
object bytes and modeled cost changes 1,632 -> 1,630. This is a removed
architectural restriction, not a claim of closing the remaining target gaps.
Stage evidence: `/tmp/qbopt-cse-chain-20260909`.

Symbolic-address checkpoint: HARR's descriptor move at 0x6f was raised as
literal zero although its immediate has a relocation to segment 5 + 6.
Raise now preserves a symbolic operand (target, offset, width and addend),
lowering retains its relocation identity, and spilling excludes these operands
from literal rematerialization. This corrects an unsound constant fact; no
runtime miscompile from that fact is claimed. Both regressions catch in-memory
restorations of the bugs; 1,015 consts/spiller checks and two focused LIR checks
pass. Strict LIR HARR/segld/matrix runtime passes across PDS /G2, QB /O and
VBDOS /G3 (nine cases). Stage dumps are in `/tmp/qbopt-symbol-harr-20260909`.
Full validation remains outstanding; touched files retain baseline lint errors.
This establishes symbolic descriptor identity, not allocation extents or no-alias.

Reassessment after immediate multiply: the whole-fixture target scan still shows
large array gaps; ordinary bools/subexp configurations meet 1.5x. HARR's actual
final MIR (`/tmp/qbopt-harr-reassess-20260909/s24-mir-r02-place.txt`) stores through
F[v3_7] at 0x78, then reloads descriptor offset D[v4_5] at 0x7d to form v3_9 and
reads F[v3_9] at 0x83. They are not yet proven equal: the far store may alias the
descriptor in today's memory model. Removing the reload without proving distinct
storage is unsound. Next substantial work is allocation/descriptor provenance in
raise, exposed as object identity to MIR alias analysis. Do not treat FAR as a
no-alias promise or next-symbol bounds as object extents. Strength tinkering alone
does not remove this barrier.

Immediate-multiply checkpoint: single-result MIR products now propagate known
factors; lowering selects the existing three-source immediate representation,
avoiding a destination tie. Widening/multi-result products are unchanged. Three
regressions fail with the old propagation/lowering; six focused checks pass.
Strict LIR matrix/lngmix runtime passes on PDS /G2 with experimental reduction
(`/tmp/qbopt-immediate-multiply.jthvzG/p-g2`). No measured kernel gain yet:
experimental matrix stays 13,138 (2.12x); production stays 12,334 (1.99x).
Do not enable strength on these figures.

Constant-rematerialization checkpoint: the spiller recreates single-definition
immediate constants at uses instead of allocating a stack slot. Grouped parallel
copies and redefined values are excluded; rematerialization precedes in-place
updates of other spilled values. Thirteen spiller checks pass; the old spiller
fails the no-slot regression. Strict LIR-only runtime passes matrix, hotlop,
pressx and lngmix across PDS /G2, QB /O and VBDOS /G3 (12 cases), artifacts
`/tmp/qbopt-remat.YNcAMl`. Matrix modeled cost falls 12,500 -> 12,334 (1.99x).
The target remains 6,210; this is progress, not completion or a full gate.

Zero-fact checkpoint: identical, same-width Held operands of integer XOR/SUB
now establish zero without requiring an input fact. Six fail-first/mutation
regressions and all 998 constant checks pass. Strict LIR-only runtime transforms
(refusal raises instead of falling back) pass matrix, hotlop and pressx on PDS
/G2, QB /O and VBDOS /G3: `/tmp/qbopt-zero-verified.PNnu34`.
This is analysis capability, not a speedup claim: matrix production cost is
12,500 / 6,210 = 2.01x; strength-enabled is 13,224 = 2.13x. Harr stays 6.27x;
segld is 3.85x production / 3.91x with strength. Keep strength disabled. Constant
materialization and allocation now need improvement; do not suppress valid facts
merely to preserve old instruction selection.

Dead-byte ownership checkpoint: `_without` now checks adjacency using `covers`,
not the operation's old address. The former check made a survivor span bytes
still owned by a jump after transformations separated address and ownership.
The focused regression fails with the old function; 46 checks pass. With the
zero-fact and strength experiments enabled in memory, matrix now reports LIR
`rebuilt` and passes PDS /G2 (`/tmp/qbopt-zero-fold.ckjHrW/ownership-p-g2`).
Neither experimental production switch changed in this checkpoint.

CSE phi checkpoint: CSE now replaces phi inputs as well as operation uses when
deleting a repeated computation. A focused regression fails with the old pass;
45 related checks pass. The zero-fact experiment exposed this dangling definition
at matrix 0x79; artifacts: `/tmp/qbopt-zero-fold.ckjHrW`.
Zero folding is withdrawn, not enabled: after fixing the phi, matrix refuses LIR
emission with `0x0044: 9 bytes are claimed by more than one op`. Subsequent runtime
PASS results were fallback, not successful recompilation. Without the experiment,
matrix emission succeeds at 12,076 / 6,210 = 1.94x. Diagnose ownership before
reintroducing identical-operand XOR/SUB zero facts; 998 host checks missed it.

Two-address multiply checkpoint: the pass skipped all MULTIPLY operations even
though the single-result, two-source form reads its destination. It now inserts
the required first-factor copy, leaving widening/fixed forms alone. The old pass
fails the regression; 48 focused checks pass. Experimental reduction passes
matrix on PDS /G2 (`/tmp/qbopt-matrix-tied.CtbS1u/p-g2`) but still costs 13,144
against production 12,076 and target 6,210. Reduction remains disabled pending
profitability and broader correctness evidence; this is not a full-gate result.

Insertion-location checkpoint: setup/update operations now use their insertion
sites instead of the old product address. Tail updates retain the predecessor's
last operation address with zero-width ownership at its end, so a branch to the
next block skips the update. Stage evidence is in
`/tmp/qbopt-matrix-reducer.IRijKE/{baseline,reduced,anchored-tail}`.
Original experimental emission put setup inside loops; corrected entry jumps
now skip latch updates (0x4d -> 0x7d and 0x8f -> 0xa3).
44 focused checks pass; the old reducer fails the insertion-location regression.
Still not safe to enable: setup `zero * 20` emits `imul bx,bx` after loading 20;
trace two-address/coalescing next. The cost regression was not pure pressure.

Current-iteration checkpoint: reduction now replaces a product with a semantic
copy into its original result instead of deleting that definition and replacing
all consumers. Exit phis therefore retain the pre-increment result. This removes
the separate deletion/renaming path and leaves copy elimination to allocation.
The exit regression fails with the old reducer; 43 focused checks pass.
Experimental `matrix-p-g2` modeled cost is 14,164 versus production 12,076
(2.28x versus 1.94x, target 6,210). No enablement: inspect production-stage dumps
to explain the added cost and condition preservation before any runtime batch.

Phi-edge SSA checkpoint: isolated construction now resolves existing phi inputs
at their predecessor ends using analysis-only reads, removed from the returned
body. The regression fails with the old constructor; 42 focused checks pass.
This does not yet substitute strength-reduced exit phis: the outgoing counter
may already be incremented, whereas the original product denotes the current
iteration. Preserve that pre-increment value explicitly before wiring exit uses.

Address-use checkpoint: strength reduction and isolated SSA construction now use
one semantic substitution utility in `ssa.py`, shared with existing transforms.
Memory operands, access lists and merge inputs stay aligned with the renamed SSA
uses. The indexed-use regression fails independently with either old renamer;
41 focused checks plus two existing substitution checks pass. Existing phi-edge
replacement and lowering condition preservation still prevent enablement.

Loop-entry checkpoint: reduced-counter setup now requires a dedicated preheader;
an entry block with a bypass successor could previously introduce a memory read
on the bypass path. The focused regression fails with the old reducer and passes
with the guard; 40 related checks pass. Still outstanding before enablement:
condition preservation in lowering and complete replacement of address/phi uses.
Do not solve condition preservation by teaching MIR optimization machine flags.

Product-result checkpoint: loop reduction now requires the first semantic result
to be the only live result, following phi dependencies transitively. Previously
a high-only result could be replaced by the low recurrence, and a second live
result behind two phis was missed. Both regressions fail with the old function;
38 focused checks pass with the fix. No production switch changed.

Loop recurrence guard checkpoint: all entry values and backedge steps must agree;
one valid backedge no longer certifies the others. Shift reduction now requires a
constant, in-range count and the counter in the value operand, not the count.
Focused checks: 35 passing across induction identity, promotion and MIR-boundary
tests. Restoring the old functions independently reproduces the rejected-path and
shift regressions. Strength reduction remains disabled; this is prerequisite
correctness work, not a new speedup or a full-gate result.

1. Whole-value recognition in raise: lngmix still carries split long accumulator halves, joins and spills. Do not move register-aware widening into MIR optimization. Existing JOIN forms are not yet uniformly explicit semantic operands.
2. Sound array objects/extents, enabling scalar promotion and loop-address reuse. FAR accesses may alias DGROUP; the next named displacement is not proof of an array boundary. Runtime descriptors distinguish near, far and huge storage. Use those facts, not a blanket no-alias rule.
3. Close the measured target gaps, then run integration/commit gates. Keep fail-first symptom regressions per fix and dump adjacent stages when debugging.

Generic copy propagation plus trimming merge dependencies was tried and **reverted**: pressx printed 0 instead of 7500 on all three compilers, and some QuickBASIC runs failed to complete. Do not resurrect that shortcut. A suspected moved-store relocation defect was disproved by the emitted-object check; removing its unnecessary symbol override did not fix a runtime bug.

Runtime scope explicitly excludes /V, /W event trapping and /X resumable errors; do not spend the next round inventing event interfaces. VBDOS B$ENRA remains unestablished. LLVM LICM dedicated-exit/store-dominance rules informed store sinking; LLVM/GCC references are under /Users/alim/work/other.
# Hoisting preserves existing SSA edges

Hoisting no longer reconstructs the entire body from variable numbers after
moving invariant definitions. Their existing uses and phi edges remain valid
when the definitions move to the dominating preheader; reconstruction discarded
cross-variable accumulator phis. A regression on real LNGMIX MIR gives a phi
its own variable and checks its incoming value identities survive an actual
hoist. It failed before the fix and passes after it.

Validation: nine focused hoist/fixed-point checks pass. Strict-LIR LNGMIX,
HOTLPX and PRESSX run correctly on PDS `/G2`, QB `/O` and VBDOS `/G3` (nine
runtime passes). Stage files: `/tmp/qbopt-hoist-ssa-20260909`. Runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-hoist-ssa-6h90ojay`.
Constant DIVMOD folding remains withdrawn; its byte-ownership issue is still
open. No performance improvement is claimed for this correctness repair.
# Constant division now reaches the executable

LNGMIX's constant quotient/remainder fold to 14285 and 5. The production cost
falls from 867 to 566 against target 210 (4.13x to 2.70x, not yet complete).
The folder uses signed truncation toward zero and retains zero-divisor and
signed-overflow cases, unknown operands, and live non-result effects.

Two ownership/selection issues were exposed and repaired in the same slice:
noncontiguous push-byte ownership now travels with MIR operations through CSE
and deletion, rather than disappearing with an operation ID; replacement
constants clear the original runtime-call node. Keeping that node emitted a
bare runtime call after its arguments had disappeared and timed out. No result
from that failed run is counted as validation.

Validation: 24 focused checks pass, one existing xfail. The emitted-code
regression fails when constant folding is disabled and separately when CSE's
extra-range transfer is disabled (the original 12-byte refusal). LNGMIX and
HOTLPX pass strict LIR execution on all three compiler families (six runs).
Artifacts: `/tmp/qbopt-constant-division-final` and
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-divfold-verified-8f_6zsli`.
Remaining LNGMIX work includes redundant high-half reconstruction and memory
traffic in its accumulator; inspect these stage dumps before changing them.
# High-part extraction experiment — next boundary to repair

The remaining LNGMIX joins reconstruct the high halves of constant 14285 and
5. `mir._handing_back` currently gives them no semantic arguments or results,
so constant propagation cannot prove either zero. A trial explicit EXTRACT
operation made both facts provable, but is not retained:

- Folding extraction to a COPY left it inside the loop (LICM excludes an
  independent copy), pulling its dependent ADD/ADC chain back in as well.
  Cost rose from 566 to 616. `/tmp/qbopt-extract-trial` has every stage;
  `s58-lir-lowered.txt` shows the zero move and ADD/ADC in the loop.
- Leaving EXTRACT unfurled instead refused at `0x004b: restore is not one
  select.py can emit`. The legacy restore adapter cannot lower an operation
  with explicit semantic operands. Renaming JOIN alone is not a migration.

Next implementation needs an explicit bit-extraction lowering, preserving
flags and partial-write semantics, alongside the raise change. Then propagate
constant carry from the known ADD into ADC so the whole invariant chain folds,
instead of replacing only its first instruction with a non-hoistable copy.
The trial was removed; production remains at 566 and no runtime or speedup is
claimed for this experiment.
# Explicit extraction reaches lowering

Runtime high-part handbacks now raise as EXTRACT(source, bit offset) with an
explicit result width. The backend implements the current 32-to-high-16 shape
with a balanced push/pop/pop expansion over abstract values. It preserves
flags and leaves register assignment to allocation. Unsupported shapes refuse.
The object writer now accepts expansion instructions with no original MIR op;
these carry generated semantics and own no original bytes.

Validation: the lowering regression failed before expansion was implemented;
13 focused extraction/division/condition tests pass. LNGMIX and HOTLPX pass
strict LIR execution on PDS `/G2`, QB `/O`, and VBDOS `/G3` (six runs).
Artifacts: `/tmp/qbopt-extract-lowered` and
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-extract-lower-2duh528o`.

This is a boundary migration, not a speedup: LNGMIX currently costs 594 versus
566 before it. Next fold explicit extraction plus known carry-dependent
arithmetic together; replacing only extraction with a constant previously
stranded its dependent chain inside the loop.
# Constant extraction and carry propagation

Constant propagation now evaluates explicit bit extraction and ADD_CARRY when
the exact input condition's carry is proven by a constant ADD. Carry facts are
kept separate from value facts: knowing carry does not establish the other
condition bits. Unknown carry and insufficient source widths remain unknown.
Folding also refuses a constant fact narrower than the result it would replace.

LNGMIX's constant ADD/ADC becomes constants; propagation into the remaining ADC
reduces cost from 594 to 580 (still 2.76x target). Fifteen focused checks pass;
carry tests failed before implementation and the width guard test fails when
the guard is removed. LNGMIX/HOTLPX pass on PDS, QB and VBDOS through strict LIR.
Artifacts: `/tmp/qbopt-carry-final`,
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-carry-tmu2qjhw` (PDS/QB),
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-carry-v-ak5up4y6` (VBDOS).

Next blocker: dead-code deletion groups operations by original address. A live
operation therefore keeps dead sibling copies. A trial deleting by object
identity instead refused with `0x005f: 12 bytes are claimed by more than one
op`; it was withdrawn. Ownership transfer must handle these siblings before
the newly constant chain can be fully removed. Do not loosen layout's check.
# Inserted moves own no disjoint input ranges

The shared-address deletion experiment's duplicate 12-byte ownership was
introduced by `objwrite._carried`, not the MIR deletion itself. Stage-by-stage
range counting found no overlapping ownership through optimization. An inserted
allocator move then inherited its parent operation's `extra_covers`, despite
correctly clearing its ordinary span and ID. It now clears the extra ranges too.
The targeted regression fails before the fix; ten extraction/division checks
pass after it. Production LNGMIX still emits at cost 580.

Deleting dead siblings by object identity now emits, but costs 598 and still
retains dead copies whose bytes have no adjacent taker. That deletion change
was not retained. `/tmp/qbopt-dce-identity` records its stages. The next change
should unify ownership of removed operations instead of relying on an adjacent
instruction's single span, allowing true deletion rather than leftover moves.
# Dead computations emit no bytes

Dead-code removal now replaces each dead operation with an empty ownership
marker: no values, operands or original node, but the same input byte ranges.
Lowering and selection give that marker an empty encoding. A live sibling at
the same input address no longer keeps the dead computation alive, and no
adjacent survivor is needed to inherit its bytes. This retains layout's full
coverage check rather than bypassing it.

LNGMIX drops from 580 to 552 (2.63x target). MATRIX measures 11376/6210
(1.83x). The regression fails against the old DCE implementation; seventeen
focused tests and nine strict-LIR runtime runs pass: LNGMIX, HOTLPX, MATRIX
on PDS `/G2`, QB `/O`, VBDOS `/G3`. Artifacts:
`/tmp/qbopt-dead-marker-final` and
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-dead-marker-qhdaewbz`.
The target is still unmet; accumulator memory traffic remains in LNGMIX.
# Promote carry-arithmetic memory reads

ADD_CARRY was missing from promotion's supported read operations. It now reuses
available stored values under the same alias/width checks as ADD, preserving
its condition input. LNGMIX's high accumulator read becomes an SSA value and
the loop emits `adc di,0` rather than reading its high word from memory.
Cost drops 552 to 540 (2.57x target).

The real-fixture regression failed first. Ten promotion checks and nine strict
LIR runtime runs pass (LNGMIX, HOTLPX, MATRIX on PDS, QB, VBDOS). Artifacts:
`/tmp/qbopt-promote-carry-final` and
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-promote-carry-m80cp_0f`.

Both accumulator stores remain in the latch. Store sinking currently considers
only stores in the block that has the exit edge, which is the test/header in
this rotated loop, not the latch. Extending it requires selecting the exit
phi's value and proving zero-trip behavior, not moving the latch value directly
to a path where it may never have been defined. The two stack-local temporary
stores and counter-copy traffic also remain visible in the emitted dump.
# Rotated-loop accumulator stores sink to the exit

Store sinking now handles a two-block rotated loop with one latch and one
exit. The moved store reads the header phi, not the latch definition. Its
outside incoming value must match the last initializing store in the entry
predecessor; unknown/aliasing writes, observers, missing initialization and
alternate latch entries refuse the move. COPY tracing respects result widths.

LNGMIX falls from 540 to 428 (2.04x target): both accumulator stores are after
the loop. The two temporary stack stores and counter-copy traffic remain.
The positive real-fixture test failed before implementation; missing-zero-trip
initialization remains a negative case. Eleven focused checks pass. Nine
strict-LIR runtime runs pass (LNGMIX, HOTLOP, MATRIX on PDS, QB, VBDOS).
Artifacts: `/tmp/qbopt-rotated-store-final` and
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-rotated-store-36lii9ld`.
# Phi widths enable counter-copy propagation

The CSE/copy-propagation width map previously described only operation results,
never phi results. It now reaches a fixed point across phis whose incoming
definitions all have the same known width. Unknown or conflicting widths stay
unknown; demanded-high-half checks still apply before substitution.

LNGMIX falls from 428 to 386 (1.84x target). The emitted loop counter now stays
in one register through increment and comparison; its two per-iteration copies
are gone. The positive width test failed first, the conflicting-width negative
passes, and fifteen focused checks plus nine strict-LIR runtime runs pass
(LNGMIX, HOTLOP, MATRIX across PDS, QB, VBDOS).
Artifacts: `/tmp/qbopt-phi-width-final` and
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-phi-width-99x3f49l`.
Two temporary stack stores and the split-word accumulator arithmetic remain.

# Nbody: correctness baseline and the next optimization boundary

At `051de97`, strict optimized LIR passes all 24 nbody outputs on PDS `/G2`,
QuickBASIC `/O`, and VBDOS `/G3`. This follows fixes for lost coalescing pins,
address-keyed dead-store deletion, duplicate widening, and undeclared restore
clobbers. NEGNOT and LNGMIX also pass on all three. This is a correctness
baseline, not evidence that the modern-backend performance goal is met.

The PDS stage dump at `/tmp/qbopt-nbody-pds-baseline` shows five runtime calls
in the interaction loop that `calls.sites()` recognizes with `consume`
arguments but an empty `pushed` classification. `mir._sites()` skips them:

- `0x1a0`: divide distance by 262144.
- `0x1b2`: divide 512 by the computed denominator.
- `0x1cd`: multiply deltaX by falloff.
- `0x1d4` and `0x204`: divide the respective products by 512.

The emitted loop spills `other` at `[bp-0x24]` and both halves of accX/accY
at `[bp-0x26]` through `[bp-0x2c]`. Array-index shifts and invariant position
reads remain too, but the arithmetic calls are an upstream constraint on
retaining those values.

Next: represent stack-fed runtime arithmetic as semantic MIR at the raise.
Resolve each argument's actual pushed value, including arguments pushed before
a nested call; combine high/low words with their exact width and ordering.
Do not merely inline stack pops into an opaque machine sequence: that hides
constant division and value lifetimes from the optimizer again. Preserve
stack balance and return-half consumers, then verify nbody and one focused
nested-call regression before measuring the resulting loop. No hand-derived
nbody target exists yet, so no modern-target ratio is established.

## Nbody: recovered arithmetic and constant division

`f921137` and `f2f5d57` recover stack-fed division and multiplication into
scalar MIR, including arguments pushed before nested calls. `3a8ac25`
propagates constants through word concatenation and removes stale machine
metadata from folded computations (CHAIN's MODMOD otherwise became 92344
instead of 13106).

Positive power-of-two scalar division now expands in algebraic MIR to a
sign-derived bias, addition and arithmetic shift; remainder is reconstructed
as dividend minus quotient times divisor and removed when unused. No register
or encoding is chosen by this transformation. Both negative inputs and the
minimum signed value preserve truncation toward zero.

PDS nbody's divisions at 0x1a0 and 0x1d4 become these sequences. With only
this transformation disabled, the weighted cost is 1,094,861; enabled it is
1,012,861 (7.5% lower). This is the opportunity model, not measured hardware
cycles or a ratio against a hand-derived target. Stage dumps:
`/tmp/qbopt-nbody-powdiv`. Strict LIR runtime checks pass nbody (24 outputs),
chain (7), and divmod (20), each on p-g2, q-O and v-g3. Focused regressions
were observed failing with the transformation disabled.

Remaining: loop-invariant current-body position loads, array-address
induction, live accumulators, the still-opaque arithmetic call, and a
hand-derived nbody target. The overall modern-compiler goal is not complete.

## Nbody: final inner call recovered, loop optimization unblocked

The last inner runtime division (PDS 0x204) was retained because dead phi
cycles mentioned its unused clobber results. Raising now follows phi inputs
only from actually read results and removes the unused phis. This exposes
the arithmetic and permits LICM to hoist the current-body scaled index;
accumulator promotion can also operate across the formerly opaque call.

Three integration defects surfaced and were fixed, not bypassed:

- Division relocation matching now permits a different index SSA value when
  the relocated address itself is unchanged; different addresses still fail.
- Widening cannot move a pair above an intervening definition it consumes.
  PDS otherwise stored DELTAY before computing it (PX0=6137536, expected 1258).
- A chain ending in stores restores the last computed high-half definition,
  not the empty definition list of its last store. Restore inputs and outputs
  are declared to allocation even though the operation is opaque. Otherwise
  PDS read an uninitialized spill and printed PX0=17758202 instead of 1258.

Each defect has a regression observed failing without its fix. Strict LIR
runtime checks pass nbody (24), negnot (4), and chain (7) on p-g2, q-O and
v-g3. PDS nbody's modeled cost is now 886,143, versus 1,012,861 before this
integration (12.5% lower). This remains a model, not a hardware benchmark.
Next: hoist invariant position reads themselves, strengthen address induction,
and derive the missing nbody target. No completion claim follows from these
nine focused runtime checks.

## Shift chains versus extra induction counters

Nbody's `other*4` is already recognized as an affine recurrence. Broadening
strength reduction to replace two-shift chains with extra counters increased
modeled cost from 886,143 to 940,015; that experiment was removed. Register
pressure matters, so recognition alone is not a profitability argument.

Algebraic simplification instead combines same-width left shifts whose final
flags are unused and whose summed count is below the value width. PDS's
0x119 shift disappears and 0x11b shifts by two. Cost becomes 880,313, with
nbody and HARR passing strict LIR on all three compilers. Stage dumps are in
`/tmp/qbopt-nbody-shift-combine`. Invariant position reads remain folded into
their subtracts and are the next larger opportunity.

## Invariant reads need an index-range proof first

The current-body index is invariant, but the indexed POSX read at 0x12d
has no allocation metadata. Alias analysis consequently considers it able to
overlap DELTAX, DELTAY, DIST2, FALLOFF, both accumulators and frame temporaries.
The existing dynamic-array path proof does not cover these fixed near arrays.
Extracting arithmetic memory operands alone therefore does not hoist the
position reads. That experiment was removed; extracting comparison operands
also produced wrong programs and is not part of the implementation.

The next prerequisite is now implemented: `_last_counter` accepts a
single-latch pretested loop with internal branches, provided its sole exit
is in the header. It handles either branch orientation and still proves the
update cannot wrap. On optimized PDS nbody it proves the inner counter's last
executed value is 5; a side-exit mutation is rejected. The regression failed
under the old two-block restriction. Nbody and stride pass strict LIR on all
three compilers.

Next, propagate this counter range through index scaling and use the resulting
byte intervals in alias analysis. Do not infer extents from neighboring symbols
or declare different-looking indexed operands disjoint. Load extraction and
LICM come after that proof, with branch-entry and relocation ownership intact.

## Scoped interval analysis

`qbopt/analysis/ranges.py` propagates signed, non-wrapping intervals through copies,
adds, subtracts, multiplication and left shifts. Loop counters seed the
analysis only in the taken loop body, never in its header or outside it.
Independent enclosing-loop proofs can intersect at an inner block.

For optimized PDS nbody, both the current-body scaled index and the other-body
scaled index are proven 0..20 bytes at 0x117. The latter bound is absent at
the inner loop header and exit. A real-fixture regression was observed failing
without the analysis; focused cases reject signed overflow and masked shift
counts. This is analysis only: emitted code and runtime behavior are unchanged.
Next is consuming these intervals in alias analysis, retaining conservative
behavior for unknown segments, wrapping addresses and unproven values.

## Range-aware alias queries

Alias analysis now projects a proven near 16-bit indexed access to its covering
byte interval. Unknown segments, width mismatches and address wrap remain
conservative. LICM supplies block-scoped read-side facts to these queries.
The real nbody POSX/DELTAX regression fails when the projection is disabled;
all twelve focused interval tests pass with it enabled. Saved runtime outputs
for nbody and HARR match BC on PDS, QB and VBDOS in
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-range-alias-2382vm4p`.

This establishes an alias proof, not an nbody speedup. A separate experiment
extracting position reads from word arithmetic did hoist them, but increased
PDS nbody modeled cost from 880,313 to 905,513 and broke HARR on PDS and QB.
That experiment is removed. The next step is whole-value scalar MIR before
LICM, so two position values do not become four independently allocated words
and inhibit widening. There is still no hand-derived nbody target establishing
its distance from the 1.5x goal.

## Whole values survive split/rejoin boundaries

Before moving pair recognition, the dumps exposed an independent break in
value continuity: recovered scalar multiplication results were extracted into
two words and concatenated again before their next arithmetic operation.
Algebraic simplification now replaces an exact high/low extraction round trip
with the original 32-bit value. Different sources, offsets and widths do not
qualify; the rule has no machine dependencies.

Nbody's product at 0x1cd now feeds the signed /512 reduction at 0x1d4 directly.
The real-fixture assertion fails with this rule disabled. PDS modeled cost
falls from 880,313 to 820,313 (6.8%); this is not hardware timing or a target
ratio. Dumps: `/tmp/qbopt-recombined`. Nbody (24 cases), CHAIN (7) and HARR
(1) pass strict LIR on each of PDS, QB and VBDOS; runtime artifacts are in
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-recombined-we7utdqf`.
Position arithmetic is still word-paired before late widening; early whole
values and invariant-load motion remain the larger unfinished step.

## Early scalar position arithmetic

`raising_longs.scalar` now recognizes adjacent load, memory-arithmetic and
store pairs at the raise boundary. It produces fresh whole values and explicit
extractions for surviving half consumers. Arithmetic requires an already
recognized whole input; memory references must match in SSA base, segment and
metadata as well as adjacent byte addresses. Live half flags and full-width
readers of a half prevent recognition. Legacy widening remains for other
idioms; this is not a claim that the migration is finished.

Nbody now enters optimization with whole position loads and subtracts at
0x11d/0x12d and 0x13c/0x144, and a whole DELTAX store at 0x135. PDS modeled
cost falls from 820,313 to 792,437. Dumps: `/tmp/qbopt-early-longs`.
The real-fixture regression fails with recognition disabled; 67 focused tests
pass. Nbody, CHAIN, HARR, negnot and arridx pass strict LIR on all three
compilers across two bounded runs. Latest artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-long-guards-ypyxvyud`.
Invariant position memory operands are still inside the scalar subtracts;
exposing those as whole loads is the next step toward LICM.

## Whole invariant position loads leave the inner loop

Early scalar arithmetic now separates its memory operand into a whole LOAD
and a pure value operation. The LOAD retains relocation identity and byte
ownership; the arithmetic has neither a machine node nor a relocated operand.
LICM moves both current-body position loads to the 0xf0 preheader in round
three, after the scaled-index range becomes available. The subtracts remain
inside the inner loop and consume the hoisted whole values.

The integration regression fails with load separation disabled. The range
test now follows source identities rather than requiring optimized operations
to retain their original addresses; its interval and alias assertions remain.
PDS nbody modeled cost is 788,837, down from 792,437. Dumps are in
`/tmp/qbopt-scalar-licm`; nbody, HARR and CHAIN pass strict LIR for PDS, QB and
VBDOS in `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-scalar-licm-z36i1_nd`.
This closes the specific invariant-position-load opportunity, not the overall
nbody target. Register pressure, accumulator halves and remaining arithmetic
round trips still limit the emitted loop.

## Scalar arithmetic across runtime results and loop edges

The raise now recognizes immediate arithmetic pairs and accepts an existing
scalar's exact extractions as a whole input, not only newly recognized load
pairs. The extraction proof is shared with algebraic recombination. Nbody's
post-division +1 and accumulator additions now remain 32-bit operations;
promotion can keep the whole accumulators across loop edges. PDS modeled cost
falls from 788,837 to 607,037 (23.0%). This remains a model, not hardware timing
or proof of the missing nbody target. Dumps: `/tmp/qbopt-scalar-immediates`.

VBDOS exposed three defects, each with a fail-first regression:

- A promoted symbolic load kept its relocation after becoming a register
  copy. Symbolic fixups now require a surviving operand just like other ones.
- Emission indexed original bytes before checking whether an instruction was
  synthetic. Original interrupt bytes are inspected only for original nodes.
- Phi elimination hardcoded word copies on ordinary and split edges. Nbody
  printed PX0=285219921 instead of 1258. Copies now preserve the width required
  across the phi's connected values, including implicit operand contracts.

The raised-only VBDOS program passed before the phi fix, isolating the defect
downstream of recognition. After the fix, nbody (24 cases), HARR (1), CHAIN
(7), and negnot (4) pass strict LIR on PDS, QB and VBDOS. Runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-scalar-phi-final-6yt_a4th`.
All 75 focused tests pass. The overall target and full architecture migration
remain unfinished; this result specifically restores whole-value continuity.

## Whole stores through exact half copies

Scalar recovery now follows width-preserving word copies back to the exact
high/low extractions, refusing width changes and cycles. The raise records its
new extraction definitions and uses the shared proof for stores as well as
arithmetic. DELTAY and FALLOFF are stored whole instead of split solely for
the stores. The real-fixture assertion failed before the change.

PDS nbody modeled cost falls from 607,037 to 517,793 (14.7%). Stage dumps:
`/tmp/qbopt-whole-stores`. Nbody, HARR, CHAIN and negnot pass strict LIR on all
three compilers; artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-whole-stores-8ej78xls`.
79 focused tests pass. Memory forwarding and the remaining legacy multiply
sites still limit value continuity; a hand-derived nbody target remains due.

## Multiply sites use the scalar call-recovery path

Classified B$MUI4 sites no longer enter the frozen machine-sequence path.
They retain their original pushes through initial raising, then use the same
scalar argument-capture and result extraction as computed multiplies. Nbody's
real-fixture regression fails when the old route is restored.

Routing alone increased cost because paired memory pushes became separate word
loads and a concatenation. Adjacent high/low memory pushes with matching SSA
addresses now capture one whole load, retaining the low operand's relocation.
Separated pushes keep independent snapshots. DELTAX's full subtraction result
now feeds its later multiply directly; the older regression checks this full
dependency rather than insisting that a removed high-half concatenation exist.

PDS nbody modeled cost falls from 517,793 to 402,593 (22.2%). Dumps:
`/tmp/qbopt-whole-multiply`. Nbody, HARR, CHAIN and negnot pass strict LIR on
PDS, QB and VBDOS; artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-whole-multiply-z2y54a7y`.
This removes the remaining frozen multiply sites in nbody, not every legacy
runtime idiom in the project. Target derivation and broader migration remain.

## Unsigned dword constants and the next forwarding experiment

The encoder now converts a dword immediate's bit pattern to the signed i32
representation required by iced. CHAIN's folded 0xbffffff9 previously refused
with `mov is not one select.py can emit`; a fail-first regression checks the
exact `66 be f9 ff ff bf` encoding. PDS CHAIN passes with the fix.

The next experiment is still uncommitted: value forwarding extends SSA
lifetimes instead of requiring a provider to be live already, and uses
operation identity rather than source address. It exposes legacy division
sites that cannot consume SSA operands, so classified divides are provisionally
routed through scalar recovery too. Nbody passes and models 388,393, but this
is **not an accepted milestone**: DIVMOD refuses an inserted multiply crossing
a live condition (PDS 0x20a). Retaining the multiply's own flag definition did
not resolve it, indicating another live condition, and that attempted change
was removed. Inspect the flag SSA/exceptional edges before committing this
broader migration. Dumps: `/tmp/qbopt-forward-divmod`; runtime evidence:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-forward-fixes-i44yb65c`.

## Call flag inputs follow established contracts

DIVMOD's refusal came from undefined condition values already present after
raising: scalar recovery removed arithmetic-call flags, while PRINT retained
reads of them. Error-handler edges propagated those phantom inputs around the
body. Raising unconditionally added FLAGS even to an established empty input
contract. It now uses the declared inputs, including FLAGS when explicitly
listed; unknown inputs remain conservative. A fail-first regression checks
the PRINT symptom and both explicit and unknown flag-input boundaries.

With the pending forwarding/divide experiment present, DIVMOD (20 cases),
nbody (24), CHAIN (7), and HARR (1) pass strict LIR on all three compilers:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-contract-flags-pziqn7pl`.
Stage dumps: `/tmp/qbopt-contract-flags`. Focused runtime/raising checks pass
259 tests, with one old divide-relocation regression still tied to the legacy
representation. This commit isolates the contract fix; the broader migration
and that regression's replacement remain pending.

## Forward values beyond BC's statement lifetimes

Memory forwarding now adds SSA uses even when the provider was not already
live. Allocation owns the resulting lifetime, including preservation across
calls. Forwarding identifies operations rather than source addresses, so two
captures at one address cannot substitute each other's operands. Classified
divide helpers now use scalar argument recovery alongside multiplies, allowing
them to consume the forwarded values instead of requiring frozen operands.

Both new regressions failed with the old behavior restored. The legacy
divide-relocation guard still checks index-value renaming and rejects a changed
address, using the real argument capture and an explicit original-operand
snapshot. Fourteen focused checks pass. The strict three-compiler runtime run
recorded above covers this implementation: DIVMOD, nbody, CHAIN, and HARR all
pass. PDS nbody now models 388,342 cycles, versus 402,593 at the prior scalar
multiply milestone; this is not hardware timing or a target-completion claim.

## Pending whole constant-store recognition

Current experiment combines adjacent constant word stores in raising, with
exact address/SSA equality and contiguous coverage. Nbody's ACCX/ACCY zero
initializers then have the same whole width as their updates, enabling
promotion and loop phis. The initializer regression failed before the change;
11 focused recognition tests pass, including base/address/gap exclusions.

Not accepted yet: nbody's modeled cost rises from 388,342 to 418,616. The
allocation diff shows both accumulator phis spilled, with additional edge
copies rather than register-resident accumulators. Compare
`/tmp/qbopt-accumulator-current` and `/tmp/qbopt-accumulator-whole`.
Nbody and HARR pass all three compilers, but CHAIN and DIVMOD refuse immediate
stores on PDS/QB (CHAIN 0x48; DIVMOD 0x9a/0x9c). VBDOS passes all four.
Runtime artifacts: `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-whole-initializers-x6ij0ddp`.
Next: inspect the immediate-store encoding refusal, then eliminate the
promoted-phi spill/edge-copy overhead. The source experiment is uncommitted.

## Dword constant stores accept their full bit patterns

CHAIN's refused initializer was `mov [seg:5+0xe],0xc1747c23`, not an
unsupported machine form. The encoder passed that positive bit pattern to
iced's signed-i32 constructor. Loads and stores now share the conversion to
the equivalent signed value. The exact-byte store regression failed before
the fix; both immediate load/store tests pass afterwards.

With whole initializers still experimental, CHAIN (7 cases) and DIVMOD (20)
pass strict LIR on PDS, QB and VBDOS:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-store-immediates-eqtnx5t9`.
The promotion cost regression remains: global accumulator stores survive
alongside private spill-slot loads/stores and phi edge transfers. This encoder
fix is committed independently; initializer recognition remains uncommitted.

## Pending conditional store sinking

Nbody's update stores are in a conditional body, not its latch, and POSX/POSY
reads appeared to alias ACCX/ACCY without scoped index intervals. The experiment
passes those intervals to alias queries and proves the exit value by tracing
the header phi's latch input back through conditional paths to matching stores,
including the initialized entry path. The invariant-only fallback remains
restricted to latch stores; a nonempty loop does not imply a conditional store
executes. The real Nbody sinking regression failed before this change.

Both accumulator writes now move to exit 0x227. Modeled cost improves from
418,616 to 409,016, still worse than committed 388,342 because private phi-spill
traffic remains. Dumps: `/tmp/qbopt-conditional-sink`. Nbody, HARR, LNGMXX and
NESTED pass strict LIR on all three compilers in
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-conditional-sink-y6v0bvte`.
26 focused tests pass, but LNGMXX's nonempty invariant-temporary sinking check
still fails: its remaining whole temporary's provider is not classified
invariant. Do not weaken that check. Source changes remain uncommitted pending
that investigation, conditional-path boundary checks, and allocator improvement.

## Scalar divisor constants reach LICM

LNGMXX's remaining temporary came from a scalar divide by a value known to
be 7. Constant propagation excluded DIVMOD operands, so LICM could not prove
the operation nonfaulting. Divisor propagation now preserves operand order
and requires a width-complete fact. Lowering materializes a constant divisor
as an abstract temporary before the divide, leaving allocation to place it.

Three fail-first checks cover 7, zero, and -1, including insufficient-width
facts; the latter two divisors remain unsafe to speculate. All 16 loop-motion
checks pass, including the previously failing invariant-store check. The
dump in `/tmp/qbopt-divisor-constants` places the divide at preheader 0x4c.
LNGMXX, DIVMOD, and nbody pass strict LIR on all three compilers in
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-divisor-constants-dbsfgqw2`.
This fix is separate from the uncommitted initializer/conditional-sinking
experiment, whose nbody cost remains 409,016 pending allocator work.

## Allocator evidence for the remaining promotion regression

Processing coalescing blocks by descending loop depth changed no modeled
cost (409,016), so that experiment was removed. Instrumenting the first
allocation, rather than guessing from final spills, identifies accumulator
intervals 556/557 with weights 14.04/11.42 and sizes 185/239 slots. Neither
has a fixed register or a clobber-mask conflict in any of the six registers.
They lose to shorter-lived intervals: ESI's sole overlapping assigned interval
501 weighs 46.67; EDI's 509 weighs 58.82. The allocation dump shows EDI
holding the constant 512 before the scalar divide.

The spiller already recognizes and rematerializes single-definition constants,
but allocation weights only count weighted references divided by interval
length, with no rematerialization discount. Next investigate that cost mismatch
and the competing ESI interval before changing allocation policy. Do not infer
that discounting constants alone will resolve the two accumulator spills.

## Whole accumulators and direct fixed-input rematerialization

LLVM's `CalcSpillWeights.cpp` halves rematerializable intervals' weights.
Trying that exact discount here changed no nbody cost, so it was removed.
The useful change is structural: constraint preparation now materializes a
proven constant directly into its required temporary instead of copying from
a separate live value. It uses the spiller's existing single-definition,
width-complete, nonrelocated constant proof. Its regression failed when the
old copy behavior was restored.

Together with whole constant-store recognition and conditional store sinking,
PDS nbody models 379,016, below the committed 388,342 baseline and the
409,016 intermediate. Dumps in `/tmp/qbopt-fixed-remat` show immediate 512
loads directly into EAX rather than preserving a separate constant in EDI.
Redundant preparations remain visible and are a later cleanup opportunity,
not hidden in the result. This is modeled cost, not hardware timing.

All 38 focused constraint, loop-motion, and whole-recognition checks pass.
Conditional sinking is refused without the matching initialization or without
either available alias proof. Nbody (24), HARR (1), CHAIN (7), DIVMOD (20),
LNGMXX (1), and NESTED (1) pass strict LIR on all three compilers:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-fixed-remat-nkr3jy7h`.
The initializer/sinking experiment is now accepted with this allocation fix.
Nbody still needs its hand-derived target; the project-wide goal is not met.

## HARR descriptor addressing: symbolic analysis is not an emitted operand

The side-by-side assembly exposed `add di,[0]` at PDS output 0x66 with
no relocation. Induction composition converted a descriptor-relative read
to its alias-analysis symbolic address and handed that new operand to
strength reduction. The new instruction had no original fixup to carry.
HARR's printed sum did not detect this: forwarding already serves the sum
from the stored value, independently of where the array store lands.

Composition now retains the actual reference and requires its base to be
invariant. Generated operations include memory-address SSA uses; lowering
accepts literal displacements through those abstract bases. The read becomes
`add di,[bx+0Ah]`, preserving the descriptor pointer instead of reading DS:0.
The three emitted-code regressions failed before the fix. All 47 focused
induction tests pass; HARR and NESTED pass runtime on all three compilers.
Stage dumps: `/tmp/qbopt-harr-es` and `/tmp/qbopt-harr-address-fixed`.
Runtime artifacts: `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-harr-address-y0z6zjgj`.
HARR costs remain 2920/2936/3132 for PDS/QB/VBDOS. The opaque ES load still
needs a machine-independent address representation; this fix does not hoist it.

The same focused runtime run found MATRIX prints T=190 instead of T=380 on
all three compilers. Replacing the changed functions in memory with HEAD's
pre-fix definitions reproduces the PDS failure independently, in
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-matrix-baseline-xgc4dvts`.
Its earlier 1.31x score is therefore not evidence of goal completion. Fix
this existing correctness failure before further optimization.

## MATRIX recurrence multiplier: retain the load and its relocation

The next dump showed MATRIX's second loop initializing and advancing its
recurrence with unrelocated reads at DS:0. Unlike HARR's based descriptor
read, this multiplier was a direct relocated memory operand. Strength
reduction copied the Cell into newly invented arithmetic without moving the
original operation's relocation to either copy. With a zero stride the
diagonal became the first row, producing 190 instead of 380.

Strength reduction now materializes the invariant multiplier as one LOAD
in the preheader, retaining the original operand identity there, and uses
its SSA value for initialization and stride. Lowering explicitly maps an
invented LOAD to MOVE, independently of the originating multiply's machine
operation. The loop now advances with register arithmetic, not a repeated
memory read. Reductions whose stride cannot be constructed are rejected
before inserting any load.

All three new emitted-address regressions failed before the fix; all 50
focused induction tests pass afterwards. MATRIX prints 380 on PDS, QB and
VBDOS; HARR and NESTED also pass on all three. Runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-matrix-stride-adva5wll`.
Pass dumps: `/tmp/qbopt-matrix-failure` and `/tmp/qbopt-matrix-stride-fixed`.
MATRIX now models 8056/8060/8066 against 6210, about 1.30x on all three.
This restores this benchmark's correctness evidence, not project completion.

## Segment-value representation experiment

After the HARR and MATRIX address fixes, an emitted-code scan of all
available primary target fixtures (PDS /G2, QB /O, VBDOS /G3) found no
remaining unrelocated zero-word displacements. This checks that specific
failure shape only, not correctness of all relocations or runtime outputs.

A raising experiment separated `es := [descriptor+2]` into an ordinary
word LOAD and an opaque resource installation. CSE and LICM did hoist the
ordinary load out of both HARR loops, as intended. The emitted result also
kept the selector in AX for the entire nest, spilled the row counter and
descriptor pointer, and installed ES twice in the inner loop. Modeled PDS
cost rose from 2920 to 3072. Dumps remain in `/tmp/qbopt-harr-selector`.
The experiment was removed; no runtime correctness claim is made for it.

This is evidence against treating selector extraction alone as the completed
address migration. The next design must represent a far address's object
identity and offset in MIR and make the selector a lowering/allocation
choice, including liveness across clobbers. An opaque installation cannot
be optimized by ordinary value passes, and a permanently live GPR selector
is not a substitute for retaining the pointer in the appropriate machine
resources. Neither an ES-specific MIR hoist nor merely hiding ES's name
behind a new operation meets the boundary rule.

## Post-allocation constant-load peephole

The allocator's flexible classes still cover GPRs and addressing registers,
not address-space resources. A segment-value migration needs that capability
as well as abstract far-address operands; it is not complete.

The previously absent final peephole phase now exists after allocation,
parallel-copy expansion and frame insertion. Its first rule removes repeated
equal nonrelocated immediate MOVs into the same physical register at the
same width. Knowledge is local to a block and resets at any non-MOV or
unknown instruction. Writes invalidate all overlapping register aliases,
including AH versus EAX; memory reads are never removed. Dropped instructions
transfer their byte coverage through the existing LIR removal mechanism.
This is post-allocation encoding cleanup, not a new LIR optimization tier.

Nbody's emitted immediate-512 count drops from three to two; its regression
failed with three before the phase was connected. PDS cost drops from
379016 to 377016. All 10 focused peephole/prologue checks pass, including
partial writes, call/unknown barriers, clobbers, relocation and block edges.
Nbody's 24 cases, MATRIX and HARR pass strict LIR runtime on all three
compilers in `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-peephole-gc_sg3g2`.
Dumps: `/tmp/qbopt-nbody-peephole`, including the new final machine phase.

## Fixed-resource allocation and coalescing

A direct allocator probe corrects the earlier blanket statement that
non-GPR resources cannot be allocated: an explicitly pinned virtual value
already allocates to ES and the interference masks already honor ES
clobbers. Flexible classes remain GPR/addressing-only. The missing piece
found here was the coalescer's candidate domain: even two values pinned to
ES were intersected against the GPR set and could never coalesce.

Pinned non-GPR values without an incompatible addressing constraint now
have their explicit singleton domain. The Briggs test counts only neighbours
whose domains overlap the prospective merged class. A fail-first regression
proves an equal ES-bound copy coalesces while six GPR values are simultaneously
live, and allocation needs no spill. Additional checks retain different ES/FS
pins and reject keeping an ES value live through an ES clobber.

All 20 coalescer/peephole checks pass. Nbody (24 cases), MATRIX and HARR pass
strict LIR runtime on all three compilers in
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-resource-coalesce-jwvqpoim`.
This establishes a backend prerequisite, not HARR's far-address migration:
MIR still needs to express the address-space value and each access's
dependency on it, with resource constraints supplied only by lowering.

## Target refresh and floating-point semantic audit

The target scoreboard at f8c9e96 puts PDS FPCSEX at 4508/1340 (3.36x),
FPCSE at 4386/1340 (3.27x), SPILL at 3206/1122 (2.86x), PRESSX at
710/308 (2.31x), and HARR at 2920/1834 (1.59x). These are modeled costs.
Numerous event-enabled PDS/VBDOS fixtures remain unmeasured because LIR
emission refuses them; direct checks of hotlop-p-evt and harr-p-evt report
an unestablished call interface, not an optimization success.

FPCSEX dumps in `/tmp/qbopt-fpcsex-current` show the repeated loads and
arithmetic still naming st0 rather than floating SSA identities. Its target
listing also reassociates the accumulator and bypasses SINGLE rounding.
An exact integer simulation of a 64-bit significand gives different results
for `(2^65 + -2^65) + 1` and `(-2^65 + 1) + 2^65` (1 versus 0).
The target document now flags that defect without changing the denominator
to make the score pass. Next FP work needs explicit value types and rounding
semantics before CSE/LICM, followed by a valid hand-derived target. Nbody's
target derivation and event-interface coverage remain separate open work.

### HARR: one address-space value across the loops

`raising_addresses` now names a near-memory ES selector load as an SSA
value and attaches that value to the following far-memory references.
Unknown clobbers end this local binding; block exits retain its observable
state. Lowering, not a MIR pass, constrains the live selector definitions
to ES. This is a bounded step, not a completed migration of all address
spaces or a cross-block resource-SSA construction.

Two integration defects surfaced in adjacent stage dumps. Complete narrow
copies were retained as though they preserved a high half, so equivalent
selectors prevented store-to-load forwarding. CSE now distinguishes a whole
copy from a partial write with `merges`. Lowering also collected deleted
origin entries as pins; a reused numeric ID pinned HARR's accumulator to ES.
Only current definitions now contribute selector pins.

HARR's primary PDS/QB/VBDOS modeled costs are **2326/2342/2538**, down from
2920/2936/3132: **1.27x/1.28x/1.38x** against the unchanged 1834 target.
The raw emitted loop contains a far store, accumulator add, and striding
induction variables, with no selector reload or array read. Stage dumps:
`/tmp/qbopt-harr-selector-live-pins`. These are modeled costs, not timings.

The regression verifies that the selector load lies outside every backward
branch interval and that the far read is eliminated on all three compilers;
disabling the raise makes all three cases fail. A clobber test and a
fail-first narrow-copy test cover the boundaries, including preserving a
genuine high-half merge. Focused tests: 27 passed. Seven runtime programs
(HARR, MATRIX, NBODY, LNGMIX, NESTED, PRESS, HOTLOP) pass on all three
compilers through strict LIR emission, including all 24 NBODY cases per
compiler. Runtime artifacts: `qbopt-selector-copies-n7sza3j1` under the
system temporary directory. The full transform test file still has nine
failures, reproduced with its pre-change CSE function; it is not a green
suite. HARR reaching its primary target does not complete the project goal.

### NESTED: combine integer address scales

The next raw loop inspection found `(row * 6) * 2` surviving as a multiply
and shift, with an extra temporary. Algebraic simplification now combines
single-use MUL/SHL scales at the same modular integer width. It does not
reassociate floating arithmetic, narrow a result, discard live flags or
high-half merges, or duplicate a shared producer. Lowering still chooses
the instruction. NESTED now emits `imul ...,12` instead of the multiply by
six followed by a shift; the previous spill slot disappears as well.

Primary modeled costs are **1202/1206/1212** (PDS/QB/VBDOS), against 768:
**1.57x/1.57x/1.58x**, still above goal. PDS was 1292 before this change.
Before/after stage directories are `/tmp/qbopt-nested-next` and
`/tmp/qbopt-nested-scales`. All three real-object regression cases failed
before the rewrite and passed after it. Algebraic and address-space tests:
67 passed, including modular overflow and refusal boundaries. Strict LIR
runtime runs of NESTED, MATRIX, HARR and all 24 NBODY cases passed on each
compiler; artifacts are `qbopt-scale-chain-brgqsuv7` in system temporary
storage. The remaining outer-loop multiplications are still visible; the
existing outer strength-reduction guard is pressure-related and has not
been removed on the strength of this result.

### Outer recurrences: remeasure the pressure guard

A fresh probe of the existing outer-loop ban contradicted its historical
rationale. With today's algebraic simplification and allocator, enabling
outer recurrences improves all three affected programs. NESTED spills only
outside its inner loop; its row multiplications disappear. The guard is
removed, leaving allocation responsible for splitting and spilling rather
than making every outer recurrence ineligible in MIR.

Primary modeled costs (PDS/QB/VBDOS):

| Program | Before | After | After / target |
| --- | --- | --- | --- |
| NESTED | 1202 / 1206 / 1212 | 1128 / 1132 / 1138 | 1.47 / 1.47 / 1.48 |
| MATRIX | 7996 / 8000 / 8006 | 7656 / 7660 / 7666 | 1.23 / 1.23 / 1.23 |
| HARR | 2326 / 2342 / 2508 | 2246 / 2262 / 2256 | 1.22 / 1.23 / 1.23 |

The preceding scale-chain change also improved MATRIX and VBDOS HARR;
the before column above is freshly measured, not copied from older rows.
The 63 available primary target fixtures show no other modeled change and
no new unmeasured result (`/tmp/qbopt-outer-target-audit.txt`). This is not a
correctness claim over those 63 fixtures. Strict LIR runtime validation is
NESTED, MATRIX, HARR and all 24 NBODY cases on each compiler, all passing;
artifacts: `qbopt-outer-recurrences-sh7l7689` under system temporary storage.
Induction, algebraic and address-space tests: 120 passed. The new emitted
NESTED regression failed on all three compilers with the guard present.

Dumps: `/tmp/qbopt-nested-outer-probe`,
`/tmp/qbopt-matrix-outer-recurrences`, `/tmp/qbopt-harr-outer-recurrences`.
The descriptor-hoist regression now requires its read to dominate the inner
preheader, allowing it to move farther out without weakening its memory
dependency checks. Product-width testing disables strength reduction to
inspect the multiply before recurrence formation eliminates it.

### PRESSX reference audit and completion-report correctness

PRESSX's emitted loop already has only add/inc/cmp/jle. Its apparent 2.31x
gap comes from comparing a runtime-input program to PRESS's folded-constant
reference. Instruction-by-instruction modeled decomposition is input 370,
computation 226, output 114, total 710. `docs/targets.md` records the audit
without inventing a more flattering denominator. A complete hand-derived
PRESSX reference remains required; this does not declare PRESSX finished.

The scoreboard now reports known invalid references (PRESSX, FPCSE, FPCSEX)
as PROVISIONAL and returns failure regardless of their numerical ratio.
Missing targets, including NBODY, also return failure with NO TARGET.
Previously an arbitrarily cheap result or absent target could return a
successful completion report. Four fail-first cases reproduce that defect;
all 13 scoreboard tests now pass. Valid targets still show their measured
ratio; HARR remains 2246/1834 in the focused CLI check. No emitted code or
target denominator was changed in this step.

### SPILL: preserve and consume partial memory constants

SPILL initialized neighboring h3/o1 with one dword store. Updating o1 then
discarded the whole fact, including the unchanged h3=7 bytes. Memory facts
are now canonical bytes, so partial stores invalidate only potentially
overlapping bytes and equivalent initializer widths agree at CFG joins.
Unknown writes remain conservative. Reads still require every byte known.

Operand folding now uses these width-proven memory facts as well as SSA
constants. It removes the replaced memory dependency and regenerates the
instruction from its MIR operation; ordered operands are not reversed.
The real SPILL fixtures now emit `add ...,7` instead of the invariant load
on each inner iteration. PDS/QB/VBDOS modeled costs are 2806/2812/2816
(about 2.50x of 1122), with PDS down from 3206. The accumulator remains in
memory; this is not completion of SPILL's optimization work.

Fail-first evidence covers the partial-write fact and all three emitted
fixtures. Constant propagation tests passed (1036 cases before operand
integration); the focused integration selection passed 91 cases. Six
runtime programs (SPILL's two checks, NESTED, MATRIX, HARR, NBODY's 24
checks, LNGMIX) passed through strict LIR on all three compilers. Artifacts:
`qbopt-memory-constants-o7e3tm7t` in system temporary storage. Stage dumps:
`/tmp/qbopt-spill-next`, `/tmp/qbopt-spill-byte-facts`,
`/tmp/qbopt-spill-constant-operands`.

### SPILL: promote fields initialized by a packed store

Promotion now chooses the width of each cell's reads and can capture a
fully covered field of a wider constant initializer. The original memory
store remains intact, including neighboring bytes. Exact-width stores still
capture their complete value, including when a narrower field is captured
beside them; unknown overlapping writes invalidate the narrow field's
availability. Unsupported partial updates remain in memory.

`consts.initialized` provides the same contained-value proof to promotion
and loop store sinking. The latter can therefore establish the accumulator's
entry value even when its zero was part of a wider initializer, and move
the write-back out of both loops. No machine register is selected by this
work. SPILL's inner loop now contains two immediate adds, increment,
compare and branch, with no memory reads or writes. Final global stores
remain before output calls.

SPILL's PDS/QB/VBDOS modeled costs fall from 2806/2812/2816 to
**1410/1416/1420**, or **1.26x/1.26x/1.27x** against 1122. All three primary
variants now meet the target. Stage directories:
`/tmp/qbopt-spill-promoted` (before the store-sinking proof) and
`/tmp/qbopt-spill-promoted-sunk` (after it).

The three real-fixture loop-memory regressions failed before the change.
Promotion, constant-cell and induction tests: 83 passed, including a shared
wide/narrow capture and an unknown overlapping write. The older HOTLOP
initializer-preservation test was corrected to find its packed initializer
and still requires that exact original store to remain after promotion.
Strict LIR runtime checks passed on all three compilers for SPILL, HOTLOP,
NESTED, MATRIX, HARR, NBODY and LNGMIX; artifacts are
`qbopt-packed-promotion-762zykcv` in system temporary storage. This verifies
those programs, not the full corpus or project goal.

### Split initialization and narrower output reads

ADDRM's long accumulator was excluded from promotion because output pushes
read its two words separately. Supported reads now determine the candidate
width; unsupported output reads remain in memory, backed by intact stores.
Promotion uses the existing byte-level constant memory analysis to capture
a full value after split initializers, only when every byte is established.
Calls and barriers conservatively invalidate these initializer facts.

ADDRM modeled costs: PDS 1578 -> 1460, QB 1584 -> 1466, VBDOS unchanged
at 1346. The accumulator reload is gone, but its loop write-back and the
long array store/reload remain opportunities; the 754 target is not met.
Stage evidence: `/tmp/qbopt-addrm-next` and `/tmp/qbopt-addrm-captured`.

The real PDS/QB regressions and complete split-initializer case failed with
the previous implementation. All 20 promotion tests pass, including missing
bytes, unknown overlapping writes and preservation of output memory reads.
Strict LIR runtime checks passed for ADDRM, SPILL, HARR, MATRIX, NESTED,
NBODY, HOTLOP and LNGMIX on all three compilers (33 cases each).
Artifacts: `qbopt-split-initializers-u9m6172r` in system temporary storage.

### Sink the split-initialized accumulator write-back

The loop exit proof now consults complete byte-level memory facts when a
single initializer cannot establish the expected entry value. This preserves
the zero-trip requirement: missing initialization or a clobber cannot justify
an exit store. The existing value/phi proof still establishes the backedge.
Facts are computed lazily only when the single-store proof is insufficient.

ADDRM PDS/QB costs fall again, 1460/1466 -> **1346/1352**; VBDOS remains
1346. The accumulator write-back is now after the loop. The target remains
754, so roughly 1.79x is still unfinished. The remaining long-array reload
is visible directly in the emitted loop, after its two word stores.
Before/after MIR dumps: `/tmp/qbopt-addrm-captured` and `/tmp/qbopt-addrm-sunk`.

47 promotion/store-motion tests pass. The new PDS/QB complete-initialization
cases fail before this change; missing and clobbered initializers retain
the loop store across all three compilers. Strict LIR runtime checks pass
for ADDRM, SPILL, NESTED, MATRIX, HARR, HOTLOP and LNGMIX (27 cases total).
Artifacts: `qbopt-split-exit-7s7gobdb` in system temporary storage.

### Raise signed word-pair stores as whole values

ADDRM stored an integer and its CWD-produced sign word into adjacent array
words, then loaded the same long. Raising now recognizes that complete
store as a machine-independent `sign_extend` value and a single long store.
Existing forwarding removes the reload. Recognition requires the matching
source, word widths and CWD definition, plus the existing identical-address
and adjacency proof for paired stores.

Lowering selects MOVSX; selection emits its explicit operands. Only CWD/CDQ
require AX/DX: applying that old blanket EXTEND constraint to MOVSX added two
unnecessary moves, so the target constraint now distinguishes those forms.
The resulting PDS/QB/VBDOS ADDRM costs are **1206/1212/1206**, down from
1346/1352/1346, still about 1.60x against 754. Address scales remain inside
the loop. Stage diff: `/tmp/qbopt-addrm-sunk` -> `/tmp/qbopt-addrm-whole-store`.

All three real-fixture whole-store regressions fail before the change;
the MOVSX encoding regression also fails before selection support. The
combined focused run reports 1118 passes and 12 existing selection failures;
all 12 also fail with the previous raising/lowering/selection/target code
loaded in isolation (`/tmp/qbopt-signed-stores-select-baseline.txt`). This is
not a claim of a green selection suite. Strict LIR runtime checks passed
99 cases across ADDRM, SPILL, NESTED, MATRIX, HARR, HOTLOP, LNGMIX and NBODY
on all three compilers. Artifacts: `qbopt-signed-stores-g026xtr_`.

### Address-shift recurrence probe: not shipped

Allowing plain SHL address recurrences would put ADDRM at 1096/1102/1096,
within 1.5x, and improve ARRIDX from 590/594/600 to 538/542/548. However,
STRIDE regresses by 25 modeled cycles and NBODY PDS regresses from 377016
to 410356. Runtime output still passes: correctness alone misses this loss.

NBODY's existing whole-position-load regression catches the important
structural difference: a position load stays in block 0x117 instead of
hoisting to 0xf0. Its inner loop also gains a second affine counter with
step 4 alongside the step-1 counter; existing range/alias proofs must remain
useful after this transformation. Unit-step and constant-start eligibility
gates did not prevent the NBODY regression. Do not repeat those probes or
ship a fixture-specific exception. Preserve the induction/range/alias
relationships before enabling these cheaper recurrences.

All experimental source/test changes were removed. The authoritative ADDRM
cost remains 1206/1212/1206, not the attractive probe number. Probe dumps:
`/tmp/qbopt-addrm-strides`, `/tmp/qbopt-nbody-shift-probe`; runtime artifacts
`qbopt-address-recurrences-adusqjrb` (96 passing cases across three compilers).

### Share the exit counter's proven iteration count

`ranges.bounded` previously bounded only the recurrence explicitly compared
at the loop exit. An independently advanced offset has no such comparison,
so replacing a scaled index with that recurrence discarded its interval.
The analysis now derives the number of advances from a proven canonical
exit counter and applies it to the other affine header recurrences.
Each start and step must be known; all taken values and the final latch
update must fit without wrapping. Bounds remain scoped to the taken body.

With the experimental shift policy injected only for diagnosis, this
restores NBODY's 0..20 offset intervals and position-load hoisting, reducing
the probe cost from 410356 to 401956. That is still worse than 377016,
so recurrence generation remains unchanged. The remaining probe regression
needs allocation/profitability analysis, not more alias speculation.
Production NBODY remains 377016 and ADDRM 1206 on PDS.
Stage diff: `/tmp/qbopt-nbody-shift-probe` -> `/tmp/qbopt-nbody-shared-ranges`.

73 focused range/induction tests pass. The injected-recurrence real-program
range regression fails with the previous analysis. Wraparound, final-latch
overflow, decreasing and constant recurrences have focused coverage.
Strict LIR runtime checks pass 93 cases across ADDRM, ARRIDX, STRIDE,
MATRIX, NESTED, HARR and NBODY on three compilers. Artifacts:
`qbopt-shared-trip-ranges-p3em9ms0`. The preceding probe's runtime total was
also 93, not the 96 recorded above.

### Pressure audit and copied-constant rematerialization

Final MIR peak live values in the shift-recurrence probe grow from 5 to 6
in ADDRM and from 12 to 15 in NBODY's interaction nest. The range proof is
repaired; additional loop-carried values still make allocation more costly.
Discounting literal spill weights did not change either NBODY result.

The spiller now follows full-width, uniquely defined copy chains when
proving a literal can be rematerialized. Previously a copy of a known
constant acquired a frame slot and reload even though the direct constant
did not. Redefinitions, grouped operands, symbolic addresses, width changes
and unseeded copy cycles remain excluded. Use widths are collected once,
not by rescanning the body per candidate.

23 focused spiller tests pass; the copied-constant no-frame-memory test
fails with the previous spiller. All 97 compared primary fixture objects,
including NBODY's regression fixture, are byte-identical before and after.
This is a backend capability improvement, not a benchmark improvement.
The NBODY pressure regression and ADDRM's 1.60x gap remain open; do not
claim copied-constant rematerialization resolved them.

### QB long arithmetic: setup instructions were not pushes

The QB LNGMIX/LNGMXX outlier was real retained runtime work. For classified
call sites, raising reconstructed the argument list from every instruction
between setup and call, including MOV/CWD. Push grouping then rejected it,
leaving both runtime helpers in the loop. The reconstructed list now contains
only actual pushes; setup computations remain separate MIR operations.
Classified literal arguments retain their proven whole constant at capture,
instead of becoming a concatenation of opaque sign-word computations.

QB LNGMIX cost falls **10592 -> 306**, within 1.5x of its 210 target.
QB LNGMXX falls **10634 -> 373**, still above target. PDS/VBDOS are unchanged:
LNGMIX 302, LNGMXX 371. LNGMIX folds both arithmetic operations; LNGMXX
retains one shared DIVMOD. These are model costs, not hardware speedups.
Raw/stage evidence: `/tmp/qbopt-lngmix-q-gap`, `/tmp/qbopt-lngmix-q-pushes`,
`/tmp/qbopt-lngmix-q-constants`.

18 focused raising tests pass. Both new QB real-fixture regressions fail
with the previous raiser; PDS/VBDOS cases already passed. Strict LIR runtime
checks pass 147 cases across LNGMIX, LNGMXX, DIVMOD, ADDRM, NBODY and HOTLOP
on all three compilers. Artifacts: `qbopt-runtime-arguments-cmdgbxab`.

### Choose the dying operand during two-address legalization

LNGMXX's accumulator addition had the invariant first, so two-address
legalization copied the invariant into a temporary, added the accumulator,
then copied the answer back at the phi. For commutative integer ADD/AND/OR/XOR,
legalization now prefers an already-tied second operand or a dying second
operand when the first remains live. Equal widths and explicit value operands
are required; grouped and fixed-interface instructions retain their order.
This is instruction legalization before allocation, not a new LIR pass tier.

PDS/QB/VBDOS LNGMXX costs fall 371/373/371 -> **331/333/331**. HOTLPX falls
532/536/542 -> **452/456/462**, meeting its 312 target in all three variants
(1.45x/1.46x/1.48x). The 63-target-fixture audit has only these six changes,
all improvements. NBODY PDS improves slightly, 377016 -> 376416, but the
shift-recurrence probe still regresses (401156), so its policy stays disabled.
LNGMXX remains above target. Stage evidence: `/tmp/qbopt-lngmxx-next` and
`/tmp/qbopt-lngmxx-commuted`.

12 focused legalization tests pass, including three real emitted-loop tests
that fail with the previous legalization. The wider LIR tests have 37 existing
failures, reproduced with the previous two-address code in
`/tmp/qbopt-twoaddr-lir-baseline.txt`; they are not presented as green.
Strict LIR runtime checks pass for LNGMIX, LNGMXX, DIVMOD, ADDRM, NBODY,
HOTLOP, SPILL, MATRIX, NESTED and HARR (162 cases), plus HOTLPX separately.
Artifacts: `qbopt-commuted-operands-hozw1s5m`, `qbopt-commuted-hotlpx-32srgv2g`.

### Target audit correction: HOTLPX and LNGMXX remain unverified

Both runtime-input twins inherited the original program's whole-program
denominator, without their own full reference listing. HOTLPX cannot use
HOTLOP's folded product 21, and LNGMXX cannot fold its runtime dividend to
100000. Input setup and arithmetic must be derived, not copied or added
to the denominator until the output happens to pass. Closed-form loop
evaluation is also a valid optimization the reference must consider.

Their legacy 312/210 values remain unchanged but are now PROVISIONAL.
The earlier statement that HOTLPX meets its target is withdrawn. Its real
80-cycle improvement remains; no optimization was removed. The two new
scoreboard cases fail before this change because a low cost incorrectly
certifies these unestablished references. `docs/targets.md` records the
source differences and requirements for replacement reference listings.

### Whole-width recurrence starts

Induction analysis used a nonexistent `Value.wide` attribute and therefore
reported every recurrence start as 16-bit. LNGMXX's 32-bit accumulator had
a word start and a long step; a full-width copy on the backedge could hide
the recurrence altogether. The analysis now reads the backedge definition's
explicit MIR result width, requires agreement across backedges, and carries
that width into the initial value. It uses no machine origin information.

Five new cases fail against the previous analysis: direct/copied long
recurrences and the real LNGMXX accumulator on all three compilers.
The focused induction/range checks pass 78/78. All 96 primary fixture
objects emit identical bytes versus the previous implementation. This is
analysis groundwork for evaluating loop exit values, not a claimed speedup.

### Evaluate affine loop exits instead of running the loop

`loopexit.evaluated`, within the MIR strength stage, now replaces a finite,
side-effect-free two-block loop with each header recurrence's final value:
`start + count * step`. This follows the exit-value evaluation idea used by
LLVM IndVarSimplify. The control recurrence must have a proven finite,
non-wrapping count; integer accumulators retain their own modular width.
Stores, unmodelled operations, escaping latch/flag values, incomplete
recurrence descriptions, and unproved termination prevent deletion.
Zero-trip loops are currently left to other simplification, not guessed.

The real emitted-backedge assertions failed first on all three LNGMXX
fixtures. Adjacent MIR dumps at `/tmp/qbopt-lngmxx-exit-before` and
`/tmp/qbopt-lngmxx-exit-fixed` show the recurrence becoming a multiply and
the loop disappearing. An intermediate dump caught retained original bytes
on removed operations; cleared operations now explicitly own no computation,
and the rewritten jump retains the provenance needed to lower its new target.

Modeled costs (PDS/QB/VBDOS):

| Program | Before | After |
| --- | --- | --- |
| LNGMXX | 331 / 333 / 331 | 269 / 271 / 269 |
| PRESS | 230 / 234 / 240 | 146 / 150 / 156 |
| PRESSX | 710 / 714 / 720 | 646 / 650 / 656 |

These nine objects are the only changes among 96 primary fixture objects;
all retain LIR emission. LNGMXX and PRESSX references remain provisional.
Focused loop-exit/induction/range checks pass 88/88. The earlier LNGMXX
width regression now disables exit evaluation so it still examines the
recurrence itself; its width assertion is unchanged. Strict runtime checks
pass 39 cases across all three compilers, covering the three changed programs
and LNGMIX, HOTLOP, HOTLPX, HARR, MATRIX, NESTED, ADDRM and SPILL.
Artifacts: `qbopt-loop-exit-34rk4ekd` and `qbopt-loop-exit-press-_bngjbd0`
under the system temporary directory. No full test suite was run.

### Sum affine increments in closed form

Loop-exit evaluation now handles an accumulator whose increment is a linear
expression of invariant values and basic recurrences. It expands same-width
COPY/ADD/SUB/INCREMENT/DECREMENT expressions, requires the accumulator's own
coefficient to be exactly one, and sums each changing term using the exact
integer coefficient `N*(N-1)/2` before reducing modulo the value width.
Repeated expression nodes are memoized. Nonlinear recurrences and mixed-width
expressions are not guessed. No machine origin or register is consulted.

HOTLPX becomes `20*(n*k)+210`, and constant HOTLOP folds to its answer.
Six real emitted-loop regressions failed first (both programs, all three
compilers). Additional cases cover descending counters, an overflowing
triangular sum, and refusal of a doubled accumulator. Stage dumps are in
`/tmp/qbopt-hotlpx-sum-before` and `/tmp/qbopt-hotlpx-sum-after`.

| Program | Before (PDS/QB/VBDOS) | After |
| --- | --- | --- |
| HOTLOP | 332 / 336 / 342 | 128 / 132 / 138 |
| HOTLPX | 452 / 456 / 462 | 272 / 276 / 282 |
| ROTATE | 452 / 456 / 462 | 296 / 300 / 306 |
| SPILL | 1410 / 1416 / 1420 | 470 / 476 / 480 |
| SPLIT | 352 / 356 / 362 | 152 / 156 / 162 |

These 15 are the only changed objects among 96 primary fixtures, all retaining
LIR emission. Focused loop/induction/range checks pass 98/98; strict runtime
checks pass 27 cases across the three compilers for these five programs plus
LNGMXX, PRESS and PRESSX. Runtime artifacts are `qbopt-triangular-sums-gxxko6e7`
and `qbopt-triangular-others-3q1oerpo` under the system temporary directory.
These are actual numerator improvements, not reference revisions; HOTLPX's
target remains provisional, and completion still requires valid modern-compiler
reference listings (including loop evaluation where legal).

### Coalesce with the neighbour's register palette

MATRIX's diagonal loop retained `mov temporary,pointer; add temporary,42;
mov pointer,temporary`. The coalescer rejected joining them because it
counted general-register neighbours as high-degree against the pointer's
three-register palette. In an unpinned neighbourhood, a neighbour with fewer
edges than its own available-register count can be coloured last. The test
now uses that count. Pinned neighbourhoods keep the established rule.

Prioritizing loop copies was tried first and did not remove the copies;
that ordering experiment was removed. Applying the new degree test around
pins also moved costs backwards in FPDEEP/NBODY; that expansion was removed.
The final change reduces MATRIX by 80 modeled cycles on every compiler:
7408/7412/7418 -> 7328/7332/7338. ARRIDX changes register assignment without
changing cost. The other 90 primary objects and NBODY's regression object
are byte-identical to the previous implementation (97-object comparison).

Three emitted-copy regressions failed before the change and pass afterwards;
all 16 focused coalescing tests pass. Stage dumps are at
`/tmp/qbopt-matrix-next` and `/tmp/qbopt-matrix-palette-after`. The broader
intermediate candidate passed 111 runtime cases; the final pinned-guard
version is separately checked on the six changed MATRIX/ARRIDX variants.

### Signed widening participates in value analysis

The scoreboard still does not certify completion: ADDRM remains at
1.60--1.62x on ordinary variants, event variants remain refused or expensive,
and several reference listings are missing/provisional. No gate was relaxed.

The explicit MIR SIGN_EXTEND introduced for whole long stores previously
had no constant or interval semantics. Constant propagation now interprets
the source's own sign bit and produces a full-width result; incomplete
source facts are insufficient. Range propagation retains the same signed
numeric interval at the wider width, rejecting intervals outside the
source's signed domain. No machine names or origin information are needed.

Eight constant/range cases failed first, as did the three real ADDRM
word-to-long counter-bound cases. ADDRM now retains 1..20 through the long
conversion feeding b(i). The constant cases also assert that the fold pass
actually replaces the conversion with a constant copy. The focused checks
pass 35/35, followed by the five augmented fold assertions. All 96 primary
fixture objects remain byte-identical, so this is analysis groundwork rather
than a measured execution improvement. No runtime suite was repeated for
unchanged emitted bytes.

### Retain constants across modeled sign extension

NBODY's interaction loop emitted `mov eax,512; cdq; mov eax,512; idiv ...`.
The post-allocation constant peephole forgot every fact at CDQ, although its
explicit destination is EDX. It now preserves unaffected constants across
register-only CWD/CDQ/MOVSX operations. Destination aliases and explicit
clobbers still invalidate facts; unknown operations, calls, and block edges
remain barriers. A conversion writing EAX is not treated like one merely
reading EAX.

The emitted constant-count assertion was tightened from two copies to one
and failed first, alongside the non-clobbering-extension case. All 11 focused
peephole checks pass. Dumps at `/tmp/qbopt-nbody-next` and
`/tmp/qbopt-nbody-constant-after` show the one deleted hot-loop materialization.
NBODY's modeled cost falls 376416 -> 374416. CHAIN also loses one materialization
per object: 921/933/881 -> 919/931/879. Those three CHAIN variants are the only
changed primary objects among 96. NBODY passes all 72 runtime cases across the
three compilers; CHAIN is separately checked on those same compiler variants.
Artifacts: `qbopt-nbody-constant-7k9vkbz8` and `qbopt-chain-constant-4j353rz8`
under the system temporary directory. No claim is made that NBODY's still
missing reference target has been reached.

### Reuse copied register values around sign extension

The post-allocation peephole now tracks scalar register-value snapshots as
well as literals. Equal-width register copies transfer the snapshot; writes
invalidate every alias of their destination, not other registers holding a
copy of the old value. A changed source therefore prevents a repeated copy
from disappearing, while two preserved copies still compare equal after
their original source is overwritten. Memory loads and segment-register
loads are not treated as reusable register copies.

LNGMXX's `mov eax,ecx; cdq; mov eax,ecx` now has one copy. Its three emitted
regressions and the basic snapshot-reuse case failed first. All 18 focused
peephole checks pass, including partial source/destination clobbers and
snapshot lifetime. `/tmp/qbopt-lngmxx-copy-after` records the final stages.
Strict runtime checks pass 156 cases across three compilers for LNGMXX,
CHAIN, DIVMOD, FPDEEP, FPEMU and MATRIX (`qbopt-copy-values-k3tpmxd3` under
the system temporary directory).

The 97-object audit (96 primary objects plus NBODY) finds only improvements:
LNGMXX 269/271/269 -> 267/269/267, CHAIN improves 12 cycles on each compiler,
MATRIX and FPEMU improve 2, FPDEEP improves 12 on PDS/VBDOS (QB unchanged).
DIVMOD loses copies too, though its very large loop-weighted score is not
an independently validated runtime cost. NBODY remains byte-identical.

### Evaluate dead accumulations without deleting observable loops

MIR exit evaluation now separates knowing a final value from deleting a loop.
It can replace a constant live-out in the uniquely reached exit and dominated
blocks while retaining the loop's stores and control. The original recurrence
must become dead after substitution; replacing a still-live loop counter only
adds materialization work and is not taken. Partial rewrites leave phi-edge
uses untouched and require an exit reached only from this loop's header.

A sign-extended basic recurrence can participate when range analysis proves
its narrow values do not wrap and its start/step are known. ADDRM's long
accumulator is therefore the constant sum 1..20 = 210, while both its word
and long array stores remain in the loop. Three real-fixture regressions
failed before the change; new guards check shared exits and accumulators
observed inside the loop. Earlier whole-body identity assertions for store
and nonlinear loops now assert exact preservation of their loop blocks,
allowing independent final-value simplifications without weakening the
no-loop-deletion requirement.

The initial attempt replaced counters still needed in their loops and raised
costs in several programs. Requiring the original recurrence to become dead
removed those regressions. Final modeled costs (PDS/QB/VBDOS):

| Program | Before | After |
| --- | --- | --- |
| ADDRM | 1206 / 1212 / 1206 | 1166 / 1172 / 1166 |
| ARRIDX | 590 / 594 / 600 | 508 / 512 / 518 |
| IVCHAN | 810 / 814 / 820 | 762 / 766 / 772 |
| LNGMIX | 302 / 306 / 302 | 282 / 286 / 282 |
| STRIDE | 569 / 573 / 579 | 527 / 531 / 537 |

These 15 objects are the only changes among 96 primary fixtures plus NBODY;
all remain LIR-emitted. Final dumps are in `/tmp/qbopt-addrm-exit-final`
(before: `/tmp/qbopt-addrm-next`). Strict runtime verification passes 18 cases
on the five changed programs across all three compilers, with artifacts at
`qbopt-partial-loop-exits-muo7dksz` under the system temporary directory.

### Reuse available address scales

ADDRM computed `i << 1` for its word array, then copied i again and shifted
by two for its long array. Algebraic simplification now reuses a smaller
same-width shift of the same SSA value in the same block. It requires the
earlier result to remain used, and preserves live flag definitions. This is
value algebra in MIR; no register or address-space names enter the pass.

The first candidate also revived unused intermediate shifts, worsening
NBODY and QB FPDEEP. Excluding those intermediates removes both regressions.
The final 97-object audit changes only ADDRM's three primary variants;
the other 94 objects, including NBODY, are byte-identical. PDS/QB/VBDOS
modeled costs fall from 1166/1172/1166 to **1126/1132/1126**. The raw loop
has two one-bit shifts with no second copy of the index. QB is still just
above 1.5 times the historical 754 target; this is not project completion.

All three emitted-code regressions failed before the change. Availability,
unused intermediates, width, flag, and modular-value checks bring the focused
algebraic file to 71 passing tests. All six strict ADDRM runtime cases pass.
Stage dumps: `/tmp/qbopt-addrm-shared-before` and
`/tmp/qbopt-addrm-shared-final`. Runtime artifacts:
`qbopt-shared-scales-_lw1cq5h` under the system temporary directory.

### Raise split long negations before optimization

NBODY's damping still extracted a whole subtraction into words and used
`neg low; adc high,0; neg high`, followed by two word stores. The raise
now recognizes that exact SSA carry chain when combining the stores and
introduces a whole-value NEG. Original word operations are left for dead
code elimination, so any separately observed flags or halves retain their
original definitions. No register knowledge is added to an optimization pass.

The emitted damping now uses one 32-bit NEG and one long store per velocity,
without the extraction push/pop sequences. The PDS NBODY fixture's modeled
cost falls from **374416 to 364016**. JUMPS improves from 6942/7064/6732
to **6482/6604/6272** on PDS/QB/VBDOS. Those four objects are the only
changes in the 97-object audit; all still use LIR emission. NBODY still has
no validated modern-compiler target and this does not establish completion.

The emitted NBODY regression fails on the preceding implementation (four
word negations instead of two long negations). Carry provenance, addend,
width, mismatched halves, and modular edge values are covered; 94 focused
tests pass. Strict runtime checks pass all 90 cases across NBODY and JUMPS
on three compilers. Before/after dumps are `/tmp/qbopt-nbody-current` and
`/tmp/qbopt-nbody-neg-whole`; runtime artifacts are
`qbopt-whole-negation-wh1x42lt` under the system temporary directory.

### Reverse negated differences in MIR

With the negation raised, NBODY's damping exposed `-(quotient - velocity)`.
Algebraic simplification now reverses a single-use, same-width integer SUB
under NEG, provided neither operation has observed flag results or partial
writes. This removes the extra negation without importing machine details.
The adjacent MIR dumps show the original SUB becoming dead and the NEG
becoming `velocity - quotient`; raw emission contains the direct subtractions.

NBODY's modeled cost falls from **364016 to 363616**. It is the only changed
object in the 97-object audit. The new emitted regression failed before the
change; the preceding whole-negation test now also checks the raised MIR
contains both whole negations, rather than requiring them to survive opt.
All 79 algebraic tests and 72 strict NBODY runtime cases pass. Dumps:
`/tmp/qbopt-nbody-neg-whole` and `/tmp/qbopt-nbody-reverse-sub`. Runtime
artifacts: `qbopt-reverse-difference-1c20xxm3` under the system temporary directory.

### NBODY's remaining temporary store: missing object identity

At 5eeabab, the optimized MIR retains the four-byte store at original
0x17f to `[bp-0x16]` (L16). Its original load at 0x187 has been forwarded;
there are **zero exact readers** in final MIR. That does not prove the store
dead: querying the existing overlap relation finds 25 call reads, 10 indexed
loads, eight argument reads, and one indirect floating load that may alias it.
These are whole-body counts, not a claim that every listed read is reachable
after this particular store. The raw emitted store remains in the inner loop.

Two separate missing proofs explain retention. `avail.dead_stores` proves
overwrite-before-read, not death at object lifetime end. `MemRef.beyond`
narrows runtime reach only inside the program data segment; it says nothing
about frame slots. `module.escaped` discovers relocated push/LEA addresses,
not frame-derived pointers. Calling an unreferenced frame displacement private
would bypass both missing proofs and is not a valid fix.

The next architectural step is explicit memory-object identity, size/lifetime,
and capture facts established during raising. Static arrays and frame objects
need distinct identities; indexed accesses need proven bounds within their
objects. Calls need reachability from actual pointer arguments and transitive
contracts, retaining unknown effects and event callbacks conservatively.
Then DSE can remove stores to unobserved, nonescaping local objects at lifetime
end. It must not infer these facts from register names inside an MIR pass.

This follows the local LLVM source's BasicAliasAnalysis::aliasCheck: different
identified underlying objects imply NoAlias; a raw numeric displacement does
not establish such an object. Source inspected from llvm-project HEAD with
`git show HEAD:llvm/lib/Analysis/BasicAliasAnalysis.cpp` (lines 1571-1580).
Acceptance requires fail-first NBODY store elimination plus retained stores
when a frame address escapes, an unknown call can inspect it, an indexed
access may reach it, or exceptional/event control flow observes it. No source
optimization or runtime behavior was changed in this investigation; the
363616 modeled NBODY cost remains the baseline.

### Floating value tracking must distinguish arithmetic results

FPCSE's MIR still contains opaque floating operands. Before using the
existing fpstack analysis to raise those into ordinary values, inspection
found that it only minted values on pushes: FADD/FMUL and unary arithmetic
never defined a new value, and an arithmetic pop discarded its result.
FLOAD also incorrectly read its destination's old value. Thus the analysis
reported the value stored after `(a+b)*c` as the initial load of a.

The tracker now resolves sources separately from destinations, creates a
new identity for arithmetic, and writes the arithmetic destination before
applying a pop. Calls and unsupported operations invalidate tracking;
stack-capacity checks prevent identities being claimed after an overflow.
Every MIR stage dump now appends the floating input/result identities,
making the incorrect chain visible without reading backward from assembly.

All seven new regression cases fail with the preceding tracker, including
FPCSE on three compilers, arithmetic-pop/unary flow, calls, stack capacity,
and the actual stage output. Together with stage tests, 11 tests pass.
New dumps are `/tmp/qbopt-fpcse-values` (before: `/tmp/qbopt-fpcse-current`).
This changes analysis and diagnostics, not generated code or measured cost.
The block-local tracker is not yet typed MIR SSA: the next step must carry
storage conversions, arithmetic precision, rounding mode, and FP effects
explicitly before enabling float CSE/LICM. In particular, an extended value
must not substitute for a SINGLE store/reload without its rounding.

### Explicit floating evaluation contracts at the raise boundary

MIR operations now carry optional machine-independent floating semantics:
input/result formats, evaluation precision, rounding, and strict exception
behavior. `raising_floats` supplies those facts from decoded shapes; later
passes need not inspect instruction names to distinguish integer conversion
from real loading or SINGLE storage from an extended intermediate.

Formats cover binary32, binary64, extended80, and signed 16/32/64-bit input
and output conversions. Arithmetic precision and rounding remain dynamic:
no default control word has been assumed. Widening loads and sign operations
need no numeric rounding but still carry strict effects. Narrow stores and
integer stores explicitly round to the destination under the environment;
unknown shapes/formats remain unannotated, not optimistically pure.

This follows LLVM's constrained-operation separation of formats, rounding,
and exception semantics (local `llvm/IR/ConstrainedOps.def` inspected).
It does not yet create floating MIR SSA values or environment effect tokens,
and grants no permission to move or CSE strict operations. Those are the next
integration steps, not claims established by this metadata change.

The three real FPCSE rounding-boundary tests failed before implementation.
Conversion-format, unary, and dump checks bring the focused set to 33 passing
tests. All 97 audited objects remain byte-identical and LIR-emitted, so no
unchanged runtime suite was repeated. Pass dumps now expose the contracts
alongside value identities: `/tmp/qbopt-fpcse-semantics`.

### Floating operands become MIR value edges

Self-contained, balanced floating blocks now raise to ordinary `Held` values
with ten-byte extended results, linked through `defines`/`uses` and SSA.
FPCSE's load, add, multiply and store no longer name st0 in MIR. Storage
operands and their rounding contracts remain explicit. Blocks with incoming
floating state, unsupported operations, or unknown stack state retain the
existing opaque representation; cross-block float phis are not implemented.

Lowering owns an identity baseline for these values and restores the original
stack operands only after checking operation order, value bindings and
evaluation semantics. Changed schedules, operands, formats or cross-block
values are refused until general floating stack allocation exists. Direct
legacy instruction consumers use the same operand check rather than treating
ten-byte values as general registers. This is an integration step, not a
floating optimization tier or a completed allocator.

The first adjacent dumps exposed generic memory forwarding substituting an
extended value for a SINGLE store/reload and LICM moving strict operations.
Both now respect floating semantics: conversion stores do not establish raw
value availability, and strict floating work cannot be hoisted or removed
as an ordinary memory store. Another integration test caught reused variable
numbers from consuming an iterator twice; fresh floating variables are now
distinct from all existing variables.

The three FPCSE value-edge regressions fail with float SSA disabled. Guards
cover changed dataflow, operation ordering, rounding, integer/float variable
separation, raw forwarding, and direct lowering. All 41 focused tests pass.
The complete preceding implementation comparison and final SSA toggle audit
both leave all 97 primary/regression objects byte-identical; there is no
performance claim and no unchanged runtime suite was repeated. Final dumps:
`/tmp/qbopt-fpcse-ssa-final`; the unsafe initial pass behavior is recorded in
`/tmp/qbopt-fpcse-ssa-probe`. Next work must replace the identity baseline
with floating scheduling/allocation and model environment effects before
changing strict evaluation order or sharing arithmetic.

### Floating lowering checks stack occupancy

Lowering now walks the actual stack of floating value identities: inputs
must occupy their required slots, arithmetic replaces a slot before popping,
pushes respect the eight-slot capacity, and live values cannot cross an
unmodelled call or leave a self-contained block. The original sequence guard
remains; this is validation, not yet a scheduler or a speedup.

Three fail-first regressions demonstrate that a missing push, premature pop,
or incorrect input slot used to pass the identity check. All 44 focused tests
pass. All 97 audited objects retain identical outcomes and bytes, so no
unchanged runtime suite was repeated. Stage dumps are in
`/tmp/qbopt-fpcse-stack-validation`. Actual floating value reuse and stack
scheduling, with rounding and environment effects preserved, remain next.

### Floating register forms are selectable

The selector now emits `fld st(i)`, `fxch st(i)`, and non-popping
add/multiply/subtract/divide (including reversed forms) with either operand
at the top. This supplies the missing instructions for future floating
allocation to duplicate a live value and compute with it without reloading
memory. The stack positions exist only on the machine side of the boundary.

Thirty-nine fail-first cases cover the new forms; three refusal cases reject
invalid slots or two non-top operands. Raw-byte expectations distinguish the
opposite subtraction/division opcode senses in the D8 and DC forms. All 86
focused floating/selection/stage tests pass, and the 97-object before/after
audit retains identical bytes and outcomes. No speedup is claimed: the
allocator does not yet request these forms, and strict rounding/effect
constraints still prevent generic floating CSE.

### Floating allocation follows current value identities

`floatalloc.placed` now assigns slots to self-contained floating value
chains from their current definitions and uses. It no longer derives those
positions from BC's saved input/output operands. Whole-body lowering accepts
consistent SSA renaming while its single-instruction legacy entry retains
the conservative identity baseline. Strict evaluation order and conversion
shapes remain guarded; missing operands, exchanges, spills and live-outs
that the allocator cannot yet handle are explicit refusals.

Three fail-first tests rename FPCSE's floating values for PDS, QB and VBDOS.
The stack-corruption tests now exercise the stack verifier directly, since
saved operand positions are no longer the allocator's authority. All 89
focused tests pass and all 97 audited objects retain identical bytes and
outcomes. This is initial allocation capability, not an optimization gain.
The floating adapter still runs before general LIR construction; integrating
floating constraints and inserted moves with the LIR allocation pipeline is
unfinished, as are cross-block values and strict-effect-aware reuse.

### Floating allocation moved behind the LIR boundary

Lowering now preserves floating SSA operands as ten-byte `ir.Held` values.
The machine pipeline's explicit `floatalloc` stage consumes LIR and assigns
stack slots before general-register allocation; the former MIR-mutating
placement adapter has been removed. Floating dependencies are removed from
the general-register graph only after their slots have been assigned. The
allocator reads machine instructions, not MIR origins or BC's saved slots.

A fail-first boundary regression verifies that FPCSE reaches LIR with
floating values and leaves floating allocation with stack operands. The
renaming and refusal tests now exercise this production pipeline. All 90
focused tests pass. The 97-object comparison with the previous lowering and
machine pipeline is byte-identical. Adjacent dumps in
`/tmp/qbopt-fpcse-lir-floatalloc` show the value-to-slot transition at
`s30-lir-lowered.txt` / `s31-lir-floatalloc.txt`, with no change to MIR.
Inserted stack moves, spills, cross-block allocation and profitable reuse
remain unfinished; this change removes the premature-placement architecture
debt rather than claiming a performance gain.

### Source organized by responsibility

Moved 68 flat modules into `objectfile`, `frontend`, `model`, `analysis`,
`optimize`, `backend`, `abi`, and `legacy`. `cycles` retains its existing
package; `flow`, `rewrite`, and `wholeseg` remain root pipeline entry points.
Imports, source-path references, tools, tests, and runtime data packaging were
updated together, with no flat compatibility wrappers. `source-layout.md`
documents ownership and the architectural debt that moving files does not fix.

Validation: 82 modules import, all 50,136 tests collect, 90 focused tests pass,
and the built wheel contains the package hierarchy plus `abi/runtime.toml`.
All 97 before/after emitted-object hashes and outcomes are identical. No
compiler behavior was changed and no runtime suite was repeated.

### Floating stack moves and live-value preservation

The LIR floating allocator now inserts `fxch` when a required input is below
the top, and `fld st(0)` when destructive arithmetic or a store would consume
a value used again later in the block. Popping subtraction keeps its operand
order when an exchange changes both positions. Inserted moves own zero
original bytes and no relocations. Eight-slot overflow and live operands of
popping arithmetic still require allocation capabilities not implemented here.

Fail-first cases cover four buried-operand forms and a producer used by both
multiply and divide. A separate emission check found inserted moves silently
became native x87 instructions beside emulator sites; assembly now preserves
the anchor's emulator mode for these register-only moves, without inheriting
its memory segment prefix. The /FPi case failed first; native opt-in is also
checked. All 98 focused floating/stage tests plus four existing emulator
encoding tests pass. All 97 audited objects retain identical bytes and outcomes.

This enables shared floating dataflow in allocation; the optimizer does not
yet create it. FPCSE's existing floating loop remains unchanged until reuse
has a sound floating-environment/rounding justification.

### Floating environment audit against LLVM and QB runtime source

The next reuse step was checked against LLVM EarlyCSE and QuickBASIC's reset
and error-handling source. QB requests x87 control word `1332h`: invalid,
divide-by-zero and overflow exceptions are unmasked. The error handler resets
the floating environment before BASIC error dispatch. LLVM EarlyCSE rejects
strict-exception arithmetic and dynamic rounding; a blanket relaxed policy
would not be modelling its strict path.

`floating-environment.md` records the sources, decoded control word, limits of
the evidence, and next proof obligations. This changes the next implementation
step to exact finite-value analysis rather than blanket floating CSE. No
compiler source changed and no tests were repeated for this evidence audit.

### Exact finite floating value analysis

`analysis.floatfacts` decodes normal finite binary32/binary64 bit patterns
and signed integers using integers and rational arithmetic. Signed zero is
retained; NaNs, infinities and subnormal storage inputs are not accepted as
these facts. Arithmetic is reported exact only if its result fits every
supported dynamic precision (24/53/64 significant bits) within the extended
exponent range. Cancellation with rounding-dependent zero sign, inexact
division, overflow/underflow and inexact destination conversions stay unknown.

The analysis consumes existing SSA and memory facts and propagates exactly
representable floating stores through an analysis-only memory transfer view.
It does not rewrite MIR. Independently established entry bytes can be supplied;
they are subject to normal write/call invalidation, not treated as immutable.
FPCSE reaches 6, 48 and 3/4 on all three compiler fixtures. QB requires explicit
constant-pool entry bytes because it loads initializers from BC_CN instead of
writing immediate bits; production optimization does not assume those bytes.

All three fixture tests fail with the analysis disabled. The focused set
passes 121 tests, and all 97 emitted-object hashes/outcomes remain unchanged.
Stage dumps expose exact numeric values (without assuming pool contents) in
`/tmp/qbopt-fpcse-exact-facts`. These are numeric proofs, not permission to
discard pending exceptions, synchronization, or floating-environment effects.
Connecting them to a sound reusable-operation proof is still required.

### Floating reuse: lowering accepts deleted computations

The whole-body floating checker now retains original ordering through empty
deletion markers rather than requiring every original floating instruction
to remain live. Markers with residual computation are rejected. The legacy
per-operation path clears a valid marker's floating provenance instead of
attempting to restore its original computation.

A real FPCSE MIR test removes the second load/add and redirects the division
to the first sum. It failed at the old sequence check; it now lowers and
allocates with three rather than four additions and a stack duplicate
preserving the sum across multiplication. The 67 focused floating lowering,
allocation and selection tests pass. This establishes backend support, not
an enabled optimization: the MIR effect proof and pipeline integration are
still outstanding, and no runtime speedup is claimed.

### CSE shares exact floating computations

The existing CSE value-numbering walk now keys floating loads/arithmetic by
MIR kind, operand value numbers, and floating semantics (including rounding,
precision and exceptions), not by instruction mnemonic. Its existing memory
overlap check still applies. Reuse is currently block-local and requires exact
numeric evidence for the producer, duplicate, and all intervening floating
work; calls, opaque operations, barriers and unknown floating effects block it.
This is deliberately not unrestricted strict-FP GVN or reassociation.

FPCSE on PDS and VBDOS now emits `fld a; fadd b; fld st0; fmul c; fstp p;
wait; fdiv c; fstp q`, removing the second load/add. SINGLE stores and the
accumulator's original addition order remain. The PDS code segment shrinks
from 198 to 191 bytes. QB's output is unchanged because its constant-pool
initializers are not established entry facts in production analysis yet.

The fail-first regression asserts both the MIR reduction and three emitted
additions rather than four. Rejection tests cover unknown effects, barriers,
aliasing writes and differing rounding semantics. 103 focused tests plus three
CSE tests pass. Of 97 audited outputs only the two FPCSE variants change;
both changed variants pass actual runtime checks (one case each). Stage dumps
are in `/tmp/qbopt-fpcse-cse`; runtime artifacts are in
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-fpcse-cse-runtime-6cjfzzr5`.

Two legacy consumers also needed their boundaries corrected: MIR instruction
classification no longer lowers floating values to ask whether they are an
instruction, and integer-pair recognition ignores typed floating operations.
Broader nonconstant floating equivalence still needs environment/effect facts;
the exact-value guard is not a substitute for that work.

### Distinguish floating unary meanings before value numbering

The raise mapped both FCHS and FABS to `FNEG`. They now raise as distinct
`FNEG` and `FABS` kinds, so a MIR consumer need not inspect a machine
mnemonic to distinguish negation from absolute value. Strict absolute-value
operations remain observable to dead-code elimination. Exact finite facts
now support both meanings, including `neg(+0) = -0`, `neg(-0) = +0`, and
`abs(-0) = +0`. The kind-distinction regression failed with the original
mapping; seven numeric cases failed before unary evaluation was added.
The focused set passes 75 tests; all 97 emitted outputs remain byte-identical
to the preceding CSE commit. This does not yet enable unrestricted
unary CSE or change the floating-environment proof requirement.

### Value numbering includes result widths

CSE keyed input widths but omitted output widths. Two `movsx` operations
reading the same byte, one producing a word and the other a dword, therefore
received the same value number. A focused MIR regression showed the dword
consumer rewritten to read four bytes from the word result. The computation
key now includes every held result's width, keeping those values distinct.
The regression failed before the fix; 31 focused tests and three existing
CSE tests pass. This is a general value-identity correction, not an FP-only
exception to the CSE rules.

### CSE retains dominance-scoped alternatives

One expression key previously held only one producer. A producer encountered
in one arm of a diamond prevented the other arm from recording its own
producer, so even two consecutive identical operations in that arm failed to
reuse a value. CSE now retains alternatives and selects the latest visited
candidate that dominates the use. Memory and floating-effect checks remain
unchanged; sibling producers cannot be used at a join they do not dominate.

Both block-order variants of the diamond regression failed before the fix.
They now reuse locally in each arm and at the join, without borrowing a
sibling value. The 33 focused tests pass. Among 97 emitted outputs, only
NBODY's object changes; its 24 PDS runtime cases pass. Its measured cost stays
363616: the final LIR diff removes seven empty markers, not seven executed
instructions, so this is not reported as a benchmark speedup. Dumps are in
`/tmp/qbopt-nbody-cse-dominance` and runtime artifacts in
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-nbody-cse-dominance-qrk0imxa`.

### CSE numbers commutative integer expressions independently of operand order

Two-input integer ADD, MUL, AND, OR, XOR, EQ and NE now use an unordered
operand pair in their value-numbering key. The operations themselves are not
reordered. Result widths, dominance, memory invalidation and observed-flag
guards still apply. SUB, DIV, shifts, ordered comparisons and strict floating
operations retain ordered keys. Seven fail-first key cases establish the
missing capability; full CSE tests verify actual substitution and the observed
flags guard. All 47 focused tests pass. All 97 audited outputs are unchanged,
so no current-suite speedup is attributed to this capability.

### Reprioritize by the remaining executed work

The current PDS target report puts FPCSE at 3956 (4386 before floating CSE)
and FPCSEX at 4508. All currently valid PDS denominators are within 1.5x;
missing and provisional references still prevent any completion claim.
The literal FPCSE source admits full exact evaluation to SINGLE 487.5.
The source-ordered rational trace, including p/q/s storage rounding, confirms
all ten iterations without assuming a rounding mode. `docs/targets.md` now
separates that destination from FPCSEX's runtime-input optimization problem.

The concrete remaining implementation boundary is memory-carried FP state:
`loopexit.evaluated` operates on header SSA phis, whereas FPCSE's accumulator
is still FLOAD/FSTORE memory state inside a block. Its exact exit value is not
a header value available to the loop-exit evaluator. Next work should expose
rounding-aware scalar memory recurrences and connect their exact exit values
to existing loop evaluation, not keep adding CSE key variants that leave the
benchmark costs unchanged. No numeric target is changed until a complete
strict reference has been constructed and verified.

### Exact evaluation of storage-rounded floating recurrences

`floatfacts.repeated` now evaluates a caller-proven count of a straight-line
MIR body against explicit entry memory facts. Each iteration has fresh local
SSA facts and carries memory bytes forward through the ordinary alias-aware
store transfer. Every floating operation and storage conversion must evaluate
exactly; no summation formula, reassociation, or host float arithmetic is used.
Unknown values/effects, calls, nonlocal addresses and inexact results refuse.
A bounded operation budget prevents compile-time runaway.

Real PDS and VBDOS FPCSE MIR bodies, seeded from their explicit pre-loop
stores, reach SINGLE `0x43f3c000` after ten iterations. The entry map is not
mutated. Focused cases reject 6/7, missing inputs, calls and invalid/excessive
counts; zero iterations preserve the entry state. The 83 focused tests pass.
Operand decoding is shared with existing scalar floating facts rather than
duplicated. This analysis is not wired to delete a loop: its caller must prove
the trip count and execution shape, and elimination must separately preserve
pending-exception synchronization and observable final memory state.

### Floating exit facts feed constant propagation

`fold` now passes independently proved floating-loop exit bytes into the
ordinary constant-propagation solver. Facts belong to a particular CFG edge:
they are applied before predecessor agreement, never at the loop header or
backedge. Subsequent aliases and runtime calls still invalidate them.

Before: an integer read of the FPCSE accumulator immediately after its loop
remained `value := load s:4`, despite the exact exit proof.
After: the focused MIR regression gets `value := 0x43f3c000` (SINGLE 487.5),
with every original strict floating operation and loop edge retained.

This is a consumer of the proof, not loop deletion. All 60 floating fixture
objects are byte-identical before/after enabling it. In literal FPCSE,
`B$PSSD` precedes the output value reads and its current contract permits
caller-memory writes, so the proof cannot cross that call. No speedup is
claimed. Stage dumps: `/tmp/qbopt-fpcse-exit-propagation`.

### Floating loop exits use proved control flow

`floatfacts.loop_exits` now connects recurrence evaluation to the existing
canonical-loop and nonwrapping induction proofs. It derives a unique positive
trip count from the actual comparison, obtains entry bytes from explicit
preheader stores, and rejects header effects that can disturb floating state.
Only the final values of floating stores are reported, not stale header memory.
No caller-supplied fixture count is needed.

Before: the numeric evaluator required a manually supplied count of ten.
After: FPCSE's MIR proves ten iterations and SINGLE 487.5; changing its bound
to three proves SINGLE 146.25. Unknown bounds, unknown header aliases and calls
refuse. The 71 focused tests pass. Each MIR stage can display the proof, e.g.
`exact loop exit 0xa9 after 10 iterations: ... D:4=0x43f3c000` in
`/tmp/qbopt-fpcse-proved-exit`. ASM and the 3956 PDS cost are unchanged: the
remaining step is consuming this proof while preserving observable effects.

### Execute one checked final floating iteration

`floatloop.specialized` consumes the exact recurrence proof. It retains the
first invariant floating load with the original initial memory/counter state,
installs only loop-carried memory needed by the last iteration, and executes
the original floating sequence once. It removes the backedge and materializes
the final integer counter. Unknown/inexact iterations, calls, outgoing flags,
and unsupported live-outs are not eligible.

FPCSE, normalized symbolic assembly:

```asm
; before: ten iterations
loop:
    fld  dword [a]
    ; shared addition, p and q computations
    fld  dword [s]
    fadd dword [p]
    fadd dword [q]
    fstp dword [s]
    wait
    inc  ax
    cmp  ax,10
    jle  loop

; after: one iteration, the same floating operation sequence
    fld  dword [a]                 ; original first exception check
    mov  dword [s],43db6000h       ; proven 438.75 before final iteration
    ; same shared addition, p and q computations
    fld  dword [s]
    fadd dword [p]
    fadd dword [q]
    fstp dword [s]                 ; 487.5
    wait
    mov  word [i],11
```

PDS/VBDOS modeled cost: **3956 -> 537**. PDS code segment grows 191 -> 204
bytes; the runtime saving comes from removing nine executions, not shrinking
the static body. QuickBASIC FPCSE remains unproved, 4598 -> 4652. PDS FPCSEX
is 4508 -> 4562: strict FP checks now prevent moving its counter store out
of the loop. Existing provisional FP target denominators remain unapproved.

The initial runtime probe printed 48.75: the inserted seed had no relocation.
Inserted stores now retain their symbolic reference provenance, and object
emission preserves multiple destinations of one relocation instead of dropping
all but one. The emitted-code regression fails with the old single-destination
behavior, then passes with the fix. DSE and store-sinking regressions likewise
failed before retaining exception-visible initial state.

112 focused FP/recurrence/loop tests and two symbolic-relocation tests pass.
The 97-primary-object before/after audit changes only FPCSE/FPCSEX across the
three compilers, with no new emission refusals. All six affected runtime cases
pass. Runtime artifacts: `qbopt-fpcse-final-relocated-226hmxj3` in the system
temporary directory. Complete stage dumps: `/tmp/qbopt-fpcse-single-checked`.

### QuickBASIC literal initialization reaches the same FP optimization

The raise now records explicit unrelocated numeric literal bytes as main-body
entry memory facts. These survive SSA rebuilding but remain mutable: aliases
and calls invalidate them, and an explicit empty initial-memory input opts
out. No procedure inherits initial loader contents. Only complete floating
reads from nonoverlapping, unrelocated BC_CN records without public symbols
qualify; the entire pool is not declared readonly.

Floating memory analysis includes exact storage conversions. QuickBASIC's
`fld literal / fstp variable` initializers therefore establish a, b, c and s,
without removing those strict FP operations. Its ordinary CSE and checked
final-iteration specialization now run with the same proof as PDS/VBDOS.

```asm
; before (initializers unchanged): ten iterations
loop:
    fld dword [a]
    ; p, q and s calculations, including a repeated a+b
    inc ax
    cmp ax,10
    jle loop

; after (initializers unchanged): one checked iteration
    fld dword [a]
    mov dword [s],43db6000h   ; 438.75
    ; shared a+b, then the original p, q and s calculation order
    mov word [i],11
```

QuickBASIC FPCSE cost: **4652 -> 749**, code segment **212 -> 218 bytes**.
The QB /O DOS run prints 487.5 and DONE. The 145-object audit (primary corpus
plus all floating variants) changes only fpcse-q-O, fpcse-q-O-zd and
fpcse-q-noO, with no new refusals. PDS/VBDOS and FPCSEX output are unchanged.
The focused checks include missing bytes, relocation-bearing records,
procedure entry, alias invalidation, SSA rebuilding and emitted seed/final
store relocation on all three compilers.

Stage dumps: `/tmp/qbopt-fpcse-q-pool-before` and
`/tmp/qbopt-fpcse-q-pool-after`. Runtime artifacts:
`qbopt-q-literal-pool-i9b_skt4` under the system temporary directory.

## Adjacent floating-point waits (2026-09-09)

The backend peephole removes an explicit WAIT immediately before another
waiting x87 instruction, within one block. Integer work, unknown instructions
and non-waiting control instructions stop the scan. This is instruction
selection cleanup, not permission for strict floating-point CSE or reassociation.
Intel SDM Vol. 1 section 8.3.12 documents the implicit pending-exception check:
https://cdrdv2-public.intel.com/835781/325462-sdm-vol-1-2abcd-3abcd-4.pdf

FPCSEX, with symbolic operands restored for readability (/FPi bytes remain
emulator instructions):

```asm
; before                       ; after
fstp dword [p]                 fstp dword [p]
wait
fld dword [a]                  fld dword [a]
; q calculation                ; q calculation
fstp dword [q]                 fstp dword [q]
wait
fld dword [s]                  fld dword [s]
; accumulation                 ; accumulation
fstp dword [s]                 fstp dword [s]
wait                           wait
inc ax                         inc ax
```

PDS FPCSEX modeled cost **4562 -> 4462**, object **989 -> 985 bytes**;
two waits per loop iteration disappear. PDS FPCSE cost **537 -> 527**,
object **937 -> 933 bytes**. QuickBASIC FPCSE cost **749 -> 724**,
object **974 -> 964 bytes**. These are modeled costs, not hardware timings;
the floating reference target is still provisional.

The 145-object audit changes 30 floating variants, all through LIR, with no
new refusals. Focused peephole tests: 27 pass. DOS validation: FPCSE and
FPCSEX on all three primary compilers, plus changed QB FPDEEP (11 cases)
and FPEMU (12 cases), all pass (29 cases total). Stage files, including
the exact prologue-to-peephole diff: `/tmp/qbopt-waits-final`.
Runtime artifacts: `qbopt-waits-runtime-u8nzjbmj` in the system temporary
directory. The major remaining FP opportunity is general value reuse across
statements, not further WAIT cleanup.
## 2026-09-10: carry dominating floating values across CFG regions

Floating allocation now gives a cross-region SSA value an owned 80-bit
frame slot, stores its definition once, and reloads local values at uses.
This supports forks, joins and loop-invariant values without rereading the
source cell or narrowing an extended value. Straight-line regions retain
their existing stack allocation. Recognition and MIR passes are unchanged.

Before: an extended value used on either arm and again after the join
was refused with `floating stack input is unavailable` (or live-out refusal).
After, the representative allocation is:

```
definition: fld tword [source]; fstp tword [owned slot]
each use:   fld tword [owned slot]; fstp tword [destination]
```

The three path regressions failed against the preceding allocator. They
execute both arms and a repeated loop path, overwrite the source between
blocks, and retain a value distinguishable only at extended precision.
Separate checks reject bypassed, pinned and multiply-defined inputs.
The focused floating/allocation-order tests pass 60/60. Existing FPCSE,
FPDEEP and FPCSEX objects are byte-identical across all three compilers;
no runtime rerun or performance improvement is claimed for those objects.
Stage dumps: `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-float-regions-3hto6b7l`.

This is a spill-based allocation baseline, not optimal cross-edge register
placement. Floating phis remain refused: they need parallel edge transfers,
including cycles and critical edges. They are the next allocation gap.
## 2026-09-10: floating phi transfers, including critical-edge cycles

Floating phi results and incoming values now use the same owned 80-bit
storage as other cross-region values. Each edge loads all its incoming
values before storing any result, so a loop-carried swap remains a swap.
The shared phi edge-placement code handles taken and fallthrough edges,
updates retained integer phi predecessors, and avoids label collisions
when another allocation phase splits an already-split body.

Before: the two-value floating swap loop was refused at its phi.
After, its backedge is selected as parallel snapshots:

```
fld tword [right slot]
fld tword [left slot]
; stack allocation exchanges as needed
fstp tword [left slot]
fstp tword [right slot]
```

The regression executes the selected stack operations along two iterations
and observes `1, 2, 2, 1`, not the serial-copy corruption `1, 2, 2, 2`.
It failed against the old allocator, covers both branch-edge directions,
and checks that subsequent integer phi elimination uses the split edge.
Inputs must have a unique dominating definition and every phi predecessor
must be covered; mixed integer/floating identities remain refused.

Floating, SSA-phi, phi-width and emission-order tests pass 71/71. The
parallel-copy module additionally has 11 passes and two pre-existing
fallback-expectation failures, reproduced on the preceding commit without
changing those tests. Nine FPCSE/FPDEEP/FPCSEX objects remain byte-identical
across QB, PDS and VBDOS. No benchmark speedup is claimed for this allocator
capability: MIR still needs to expose more cross-statement/loop reuse.
Stage dumps: `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-floating-phis-4795iz9n`.
## 2026-09-10: distinguish exact numeric FP targets from verified full programs

A scan of primary PDS fixtures found no cross-block floating CSE candidate
in the current pipeline. Rather than add an unused transformation, the
target report was refreshed and FPCSE's remaining instructions inspected.
PDS/VBDOS retain initial `s=0` and counter stores before WAIT; QB's WAIT
precedes initialization. The existing checkpoint regression requires that
initial state to remain visible to a pending exception.

The 98-unit FPCSE and 1086-unit FPDEEP listings prove numeric results and
price the retained print calls, but omit synchronization and numeric stores
without the necessary whole-program observation proof. They are now
provisional, with unchanged denominators and measured costs. This does not
improve any score or declare either program complete; it prevents an
unverified comparison from passing completion even at an arbitrarily low
cost. The two new cases failed first because the report returned success.

Seven focused target/checkpoint checks pass. Seven unrelated failures in
the broader two modules reproduce on the preceding code: NOTS's stale
1.43x expectation (now 1.33x) and six older FPCSE loop-shape expectations.
Those tests and all emitted code are unchanged. Next evidence needed is a
versioned startup/environment and observer contract, or a complete independent
target that retains the required observations—not a denominator inferred
from current output.
## 2026-09-10: propagate proven constants into ordinary MIR stores

The runtime-entry investigation was bounded at a real missing contract.
`runtime/crt/fpreset.asm` requests reset (BX=1) and control word 1332h
(BX=4); QB/PDS's shipped wrappers first test the nullable `_fpinit` vector.
VBDOS calls `__fpmath` directly, whose dispatch remains unresolved by the
bounded dependency analysis. `runtime/rt/rtinit.asm` chooses the first BASIC
module through link order and runs component initializers. None of this
establishes reset state at an arbitrary object module's entry. No new
startup or floating-environment assumption was enabled.

Instead, the inspected FPCSE code exposed a missing classic MIR rule:
constant operands were propagated into arithmetic and ARG, but not STORE.
The fold now substitutes a width-proven scalar constant while retaining
the store, its address dependencies, and every synchronization point.
No machine-specific pass was added. Before/after at the initial counter:

```
mov ax,1           -> mov word [i],1
mov [i],ax            wait
wait
```

The real PDS/VBDOS regression failed first. Width and address-use guards
are tested independently. 113 constant-store/peephole tests pass; original
and optimized FPCSE, FPDEEP and ARRIDX outputs match on QB, PDS and VBDOS.
Every stage is dumped under
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-constant-stores-slm_wbdf`.

Verified ranking changes, not hardware timings:

| Program | PDS | QB | VBDOS |
| --- | --- | --- | --- |
| FPCSE | 161 → 157 | 147 → 145 | 161 → 157 |
| FPDEEP | 1797 → 1797 | 1598 → 1592 | 1797 → 1797 |
| ARRIDX | 504 → 502 | 504 → 502 | 504 → 502 |

ARRIDX initially appeared to fall to 312, contradicted by its unchanged
loop and one removed exit move. The temporary optimized filename ARRIDXQ
selected the default loop weight rather than ARRIDX's weight. Both sides
were re-costed under identical canonical filenames in separate directories;
the table uses those results. Do not reuse the earlier 312 figure.
Some objects grow a few bytes because immediate stores use longer encodings;
the gain is fewer executed register moves, not uniformly smaller objects.
## 2026-09-10: benchmark identity survives output-file renaming

The ARRIDX naming error from the previous round is now prevented by the
instrument. Both loop weighting and target selection use the OMF THEADR
source name, normalized for DOS paths. The fixture filename convention is
only a fallback for headerless objects or report-only names.

Before, identical optimized bytes named ARRIDXQ.OBJ cost 312 while those
named arridx-p-g2.obj cost 502. After, both cost 502 and select ARRIDX's
target. A misleading HOTLOP filename also cannot select HOTLOP's denominator.
These three real-object cases failed first. Eleven focused identity,
provisional/missing-target and event checks pass; the larger scoreboard
run has only its known stale NOTS 1.43x expectation (current code is 1.31x).
No compiler output changed, and no runtime rerun was needed.
## 2026-09-10: sink literal invariant stores out of nonempty loops

HARR's inner loop already uses pointer increments. Its outer latch still
wrote the final column value `11` on every row. Constant propagation had
made that value literal, but store sinking accepted only invariant Held
values. The invariant-value rule now also accepts Const, under the same
proved-nonempty, single-latch, single-exit and no-observer/alias conditions.
The write is retained exactly once; zero-trip proof failure leaves it inside.

```
before outer latch:            after outer latch:
    mov word [c],11                inc row
    inc row                        advance row pointer
    advance row pointer        after outer exit:
                                   mov word [c],11
```

The three real HARR cases failed first with the constant store still in
the loop; their three nonempty-proof guard cases stayed unchanged. All 33
store-motion tests pass. Original and optimized HARR/LNGMXX outputs match
on QB, PDS and VBDOS. HARR's rankings fall 2048→1994 on PDS/VBDOS and
2084→2030 on QB, exactly nine fewer six-unit stores with the instrument's
ten-iteration weight. Object sizes are unchanged. LNGMXX is unchanged.
Artifacts and all stage dumps:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-constant-loop-stores-fvks_2bi`.

The NBODY assembly inspected this round still contains unabsorbed arithmetic
helpers in initialization and velocity damping, plus allocator spill traffic.
That is a concrete remaining code-generation gap, unlike HARR's already
strength-reduced address stride.
# Per-site call inputs reach SSA construction

NBODY's Y damping retained B$DVI4 although X damping became native. The
raise's arithmetic matcher accepted both; the Y call's scratch values fed
loop phis that reached PITSNAP. Its per-site contract already establishes
stack arguments and no register inputs, but SSA construction looked up a
generic name-only contract instead. Phi placement and renaming now use the
same per-site contracts as call operands and lowering. Unknown side effects
remain unknown; known inputs do not imply preserved registers or memory.

Before: push 16, push velocity, call B$DVI4, split-word subtract/negate/store.
After: native MIR division followed by signed power-of-two lowering and a
whole-value subtraction/store. Both initialization multiplies are native as
well; only PITSNAP's own multiply remains a helper. VBDOS NBODY object size
is **4469 -> 4231 bytes**. All 24 position/velocity outputs match BC's
original; timing output is deliberately excluded from equality. PROCS
original/optimized output also matches on QB, PDS and VBDOS.

The real NBODY regression failed first. Arithmetic raising passes 22 tests;
six targeted input/contract checks pass. A broader MIR run passed 3216 and
stopped on two failures reproduced with HEAD's original functions:
`test_absorbed_multiply_defines_its_returned_high_half` and the ADDRM PDS
event `test_resolving_a_body_that_has_not_moved_changes_nothing` case.

Before/after pass dumps and runtime output are under
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-site-contracts-wzq4x7x1`.
No modeled NBODY speedup is claimed: opportunity.counted returned 6386 for
both objects because re-raising either optimized object returns only the
PITSNAP body, silently dropping main. The next measurement fix must cost
all decoded reachable bodies or reject incomplete coverage, not accept a
partial score. Raw assembly and object bytes establish this change instead.

## Cost all decoded bodies independently of MIR recognition

The scorekeeper now partitions decoded instructions into procedure bodies
and costs each CFG directly. It no longer uses successful MIR raises as
the inventory of executable code. Body coverage must be complete and
nonoverlapping; otherwise measurement fails. MIR-based opportunity counters
remain separate, with an explicit count of unavailable code blocks.

The NBODY regression failed first: 6386 (only PITSNAP) versus 83642 from
all decoded bodies. Corrected before/after costs for commit 3c42800 are
**92310 -> 83642**, a **9.4%** reduction in this weighted instruction model.
Original BC output scores 1206863. These use the same default ten-trip
nesting weights and helper cost table, not measured CPU time or the actual
NBODY trip counts; no optimal-target ratio is claimed. Both optimized
objects have 25 blocks unavailable to MIR opportunity analysis, but their
instructions are fully costed now.

Four focused checks pass: actual optimized NBODY, complete absence of MIR
recognition, incomplete decoded coverage, and duplicate decoded coverage.
The initial scoreboard run passed 43 tests with only its previously known
NOTS exact-string failure (expects 1.43x, current result 1.31x). No compiler
output changes in this measurement commit; runtime evidence is unchanged.

## Remove false post-termination edges from execution costing

The decoded inventory deliberately includes bytes after calls, but that is
not an execution graph. In optimized NBODY, CEND's nominal fallthrough
reached CENP and then allocator-appended phi edges. This both put the final
output loop inside the simulation and introduced a false second entrance
to its inner loops. The earlier full-inventory numbers therefore still
had incorrect loop weights.

Costing now trims each block at its first established non-returning call,
then follows only reachable normal successors from that procedure's entry.
All decoded bytes still undergo the complete/nonoverlapping inventory
check. Call effects stay unchanged: CENP may enter user error handling and
has ANY memory effects; NEVER only removes its normal return edge.

This **supersedes the 92310 -> 83642 estimate above**. For the same two
objects, the corrected model is **403607 -> 367327 (9.0% lower)**. BC's
original scores 1206841. These remain default-ten-trip weighted estimates,
not hardware timings or a valid optimal-reference ratio for NBODY.

The real-fixture output-loop regression failed first with depth two rather
than one. The unknown-control variant retains depth two, proving no return
behavior is inferred without a contract. Twelve focused cost/coverage
checks pass. Emitted bytes and runtime outputs are unchanged.

## Commute an allocated accumulator instead of copying over it

NBODY's force loop emitted `mov esi,ecx; mov ecx,eax; add ecx,esi`.
Peephole now retains the saved accumulator and emits
`mov esi,ecx; add ecx,eax`. This preserves ESI as well as the sum and
arithmetic flags; it does not need a claim that the saved register is dead.
The same rewrite applies to AND/OR/XOR, not subtraction or carry-dependent
operations. It is restricted to matching full-width register triples with
no groups, clobbers or fixed-register interfaces. MIR remains unchanged.

The real emitted-code regression failed first. All 118 peephole checks
pass, including word/dword result and saved-register equivalence checks.
VBDOS NBODY **4231 -> 4225 bytes**, modeled **367327 -> 365127**;
PDS NBODY **2912 -> 2906 bytes**, modeled **363522 -> 361322**.
Two copies disappear in each object. Both original/optimized NBODY outputs
match, excluding only the benchmark's TICKS line. QB LNGMXX stays byte-
and cost-identical (838 bytes, 246 units), with matching runtime output.
All-stage before/after dumps and runtime files:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-commuted-accumulator-7gjcwoyy`.

## Raise captured long comparisons as ordinary MIR

NBODY's computed step counter reached B$CPI4 through two word pushes,
followed by a dword memory argument. The stack matcher knew this call,
but arithmetic raising only accepted multiply/divide/remainder. Computed
comparisons now share their argument-capture machinery and become a
flag-producing MIR comparison. Unlike multiply/divide, CPI4 pushes the
left operand first; this order is asserted in the regression. The existing
runtime-vs-native CF/PF/AF refusal still gates the transformation.

Before: `push high; push low; push dword [limit]; call B$CPI4`.
After: compare captured whole values with `cmp eax,ebx`; NBODY's timer
comparisons also become native immediate comparisons. Remaining pair
assembly around the main comparison is an allocation/whole-value gap,
not a reason to leave the comparison opaque to MIR.

VBDOS NBODY **4225 -> 4211 bytes**, modeled **365127 -> 364019**.
PDS NBODY **2906 -> 2896 bytes**, modeled **361322 -> 360874**.
Both runtime outputs match the originals (excluding only TICKS). CMPORD
matches original output and retains identical emitted bytes on QB, PDS,
and VBDOS. All 26 arithmetic-raising checks pass; the real NBODY regression
failed first and three flag-safety cases retain the helper.
Before/after stage dumps and runtime artifacts:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-captured-comparison-4t2vcg63`.

## Next whole-value boundary: loop phis and sign-fill recognition

NBODY's final MIR at 0x2f0 has separate high/low phis. The 0x2e3
backedge supplies EXTRACT(whole step+1,16/0), but the 0xc7 entry supplies
the low constant 1 and an opaque CONVERT named `cwd`. `consts.known`
therefore knows low=1 and does not know high=0. At 0x2fe the newly raised
comparison CONCATs those two phis again.

`algebraic._recombined` already removes straight-line exact extraction
round trips; adding another such rule would not fix this case. The missing
pieces are (1) normalize the remaining sign-fill conversions in the raise
without changing flags, and (2) combine matching word phis into a whole
value when every incoming edge proves its full value. The latter must stay
machine-independent; recognizing `cwd` in an optimization pass would
violate the MIR boundary. Lowering EXTRACT currently uses push/pop and
SIGN_EXTEND uses MOVSX, so those existing value operations preserve flags.
No change to generated code is claimed for this investigation.

### Sign-fill isolation: equivalent value, failing emitted program

An experimental raise of residual CWD to SIGN_EXTEND plus EXTRACT exposed
the constant high word, but is **not retained**: normalizing only NBODY's
counter seed at original 0xe3 made VBDOS NBODY time out without output.
Normalizing only the initialization conversions at 0x72 and 0x99 completed
with all 24 expected values. Host checks alone did not catch the failure.

A fresh linked-executable experiment replaced the working seed sequence
`mov ax,1; mov bx,ax; sar bx,15` with
`mov ax,1; mov bx,0; nop; nop`, preserving its length. Both executables
finished with identical 24 values and DONE (excluding TICKS).
Artifacts: `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-seed-equal-length-smywdk_l`.
This rules out the zero seed alone as sufficient to explain the failure;
it does not yet establish a relocation defect.

The failing seed-only object differs in decoded instructions only by that
replacement without padding and the corresponding two-byte target shifts.
All 164 fixups retain their targets with the expected offset adjustments;
the code-header self-reference also moves by two. LINK maps place RTCODE
at the same 0x590 in both executables. A fresh run of the failing linked
executable independently timed out after 15 seconds with empty output:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-seed-linked-recheck-w2r1_veh`.
Next diagnosis must inspect the linked execution/remaining layout-dependent
state, not repeat the constant-folding argument or enable this normalization.

### Residual sign fills raised; runtime failure is core-dependent

The identical failing linked executable completes with all 24 expected values
and DONE under DOSBox-X `core=normal`. The full normalization experiment also
passes there. Neither test relinks or changes executable bytes. Outside the
module's code, the linked images are byte-identical; their relocation-table
changes match the two-byte shift. This establishes core dependence, not the
precise emulator defect. Do not call the dynamic-core timeout a proven compiler
miscompile, or treat a timeout as a passing result. The harness default remains
unchanged; these explicit correctness runs use the normal core.

`raising_longs.sign_fills` now exposes residual CWD as SIGN_EXTEND followed by
EXTRACT of the high word. Recognition stays in the raise; optimization sees
only values. The original word result and covered instruction survive, while
the artificial old-high partial-write dependency does not. The NBODY seed
regression failed first with an unknown high word; it also checks positive,
negative and signed-word boundary seeds.

Before: `mov bx,ax; sar bx,15`. After folding the seed: `mov bx,0`.
This is a whole-value prerequisite, not yet a size win: full normalization
changes VBDOS NBODY 4211 -> 4223 bytes and PDS NBODY 2896 -> 2900 bytes;
QB CMPORD remains 3873. Fresh emitted NBODY on VBDOS and PDS and CMPORD on QB
match original outputs under the normal core, excluding only NBODY TICKS.
Every-pass dumps and linked results:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-sign-fill-verified-m2uzbzwp`.
The remaining optimization is still combining the paired loop phis into one
whole value; normalizing the seed alone does not accomplish it.

### Whole-value comparison phi, with word consumers retained

`wholephis.joined`, called by algebraic, now distributes a CONCAT over two
corresponding word phis when every incoming edge proves either exact extracts
of one whole value or two known word constants. Width-preserving copies are
followed. New predecessor copies share a fresh abstract variable, and the
comparison consumes its whole-value phi. No register, encoding or runtime
idiom is consulted. Missing edges, unrelated halves and unknown seeds refuse
the rewrite. The real NBODY regression failed first; focused checks also
verify fresh-variable identity and SSA resolution.

Before the header comparison: `concat highPhi, lowPhi`.
After: `wholePhi = phi(entry: 1, backedge: nextWhole)`, consumed directly.
The old word phis remain for their independent stores. This is not yet an
overall code-size win: VBDOS NBODY 4223 -> 4225 bytes, PDS 2900 -> 2913.
Both match all original results under the normal core; QB CMPORD remains
3873 bytes and matches. Dumps and output:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-whole-phi-checkpoint-qpize_ny`.

An extension replacing those old word phis with local EXTRACTs of the whole
phi was withdrawn: VBDOS emitted 4219 bytes but printed an unprintable runtime
error at 0825:0377 under the normal core. Its every-pass dumps and executable
are in `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-whole-phi-single-egodhrtz`.
That failure is not the earlier dynamic-core timeout. Locate its first wrong
stage before removing the remaining word consumers; do not claim the loop
now carries only one value or that the induction optimization is complete.

### Root cause: raw emission confused coverage with instruction length

The phi-removal failure was not evidence against the value rewrite. Bypassing
only the two PITSNAP calls in its existing executable restored all 24 physics
results. The timer's first `IN AL,DX` at original 0x435 owned coverage
0x432..0x436 after the preceding port-number load was removed. The emitter
used that four-byte ownership length but copied starting at 0x435, emitting
`EC 30 E4 89`: IN, the following XOR, and the next instruction's first byte.
The subsequent selected `AND AX,255` then decoded with that stray 89 as
`mov [di],sp; inc word [bx+si]`, corrupting memory. LIR still showed IN then
AND; the first wrong representation was the final byte stream.

Raw emission now uses the original node's exact span for both length and
copying. Coverage still accounts for deleted/replaced bytes, independently.
The real NBODY emitted-instruction regression fails before the fix and passes
after it. NBODY now matches on **both normal and dynamic cores**, at 4219
bytes versus 4225 before this fix, with the timing calls intact.
Artifacts: `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-opaque-span-kmtbcugt`.

This supersedes the earlier suggestion of an emulator defect: core-dependent
behavior was an observation, not a diagnosis. Unintended memory writes can
appear to work under one layout/core and fail under another. The single-phi
extension can now be retried with this emitter correction, rather than
working around the corrupted output in MIR.

### Single whole counter phi verified after the emitter fix

The word-phi replacement is now enabled: both former word results are local
EXTRACTs of the whole phi, so only one counter value crosses the incoming
edges. Its regression failed first while the two old phis remained. Eleven
focused phi/recombination/raw-emission checks pass. The legacy restore test
selected alongside the emitter regression still fails because it finds no
Restore nodes before emission; loading the pre-fix assembler reproduces that
same assertion. It was not weakened.

Fresh default-core runs match all NBODY values on VBDOS and PDS, with timing
calls enabled. QB CMPORD also matches. Against the prior whole-phi checkpoint,
NBODY is 4225 -> 4213 bytes on VBDOS and 2913 -> 2896 on PDS; CMPORD stays
3873. The VBDOS total includes six bytes removed by correcting raw emission.
Artifacts: `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-single-phi-fixed-9p427sjp`.

Before MIR: two word phis plus a separately reconstructed whole comparison.
After MIR: one whole phi; word stores extract from it; comparison uses it
directly. Emitted code still extracts twice with push/pop to store the low
and high words, then reloads the whole counter on the backedge. Combining
those adjacent stores and preserving the whole value through the loop is
the next remaining work, not something this checkpoint claims to have done.

### Whole stores enable counter promotion

Algebraic now combines consecutive word stores when their addresses are
adjacent and their values are exact low/high extracts of the same whole
value. It requires matching memory-reference attributes, no intervening
operation, no extra definitions/loads and no barrier. Coverage of both old
stores is retained separately from the selected whole store's bytes.
This is value/memory reasoning; no machine register or encoding is consulted.

The real NBODY regression failed first with two stores. After combination,
existing promotion and store elimination remove stepNo memory traffic from
the whole outer loop and retain one exit store. Fourteen focused checks pass,
including mismatched addresses, unrelated halves and non-store rejection.
Fresh NBODY runtime outputs match on VBDOS/PDS; QB CMPORD is unchanged and
matches. Every-pass dumps and linked output:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-whole-stores-3c2fucaq`.

VBDOS NBODY: 4213 -> 4209 bytes, modeled cost 364154 -> 363812.
PDS NBODY: 2896 -> 2892 bytes, modeled cost 360989 -> 360647.
These are the existing loop-weighted estimates, not measured CPU timings.
The remaining generated counter is allocated to a stack slot: load, increment,
spill, reload for comparison. Before this change it reconstructed two halves
and wrote the global counter every iteration. MIR now has one recurrence
without those global accesses; keeping it allocated profitably across the
nested loops remains a backend opportunity.

### Remove the counter reload already supplied by every incoming edge

The post-allocation peephole now removes a block-entry allocator-owned reload
only when every immediate predecessor ends by storing the exact same physical
register into the exact same slot. Only empty instructions and direct branches
may follow that store. The entry block is excluded; differing slots, widths,
registers, missing stores, clobbers and ordinary source-program loads retain
the reload. This is a machine-assignment cleanup after allocation, not a new
LIR optimization tier or an optimization-pass register preference.

NBODY's preheader and backedge both end with `mov [bp-24h],eax`. Before,
the header immediately did `mov eax,[bp-24h]`; after, it directly loads the
bound and compares EAX. The inner-loop spill remains necessary under the
current allocation; no claim is made that the counter stays in a register
through those loops.

The real emitted-LIR regression failed first. All 127 peephole checks pass.
VBDOS NBODY is 4209 -> 4205 bytes, modeled cost 363812 -> 363752; PDS is
2892 -> 2888, cost 360647 -> 360587. Both runtime outputs match. QB LNGMXX
matches at unchanged 838 bytes / 246 modeled cost. Dumps and output:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-edge-reload-c2reus8k`.

### Reuse signed whole values instead of extracting and rebuilding them

The exact-word reconstruction proof now also recognizes
`concat(extract(sign_extend(x),16), x)` as the already available signed whole
value, following the existing width-preserving copies. The extension must
consume that exact low word and produce the exact four-byte value; a different
source, result width, operation kind or extraction offset is rejected.
This is a MIR value identity, with no machine-specific cost or register rule.

Before NBODY initialization: MOVSX, PUSH/POP/POP to obtain the high word,
PUSH/PUSH/POP to reconstruct the long, then SHL. After: MOVSX then SHL.
The real fixture regression failed first; 54 raising/initialization checks
and 11 focused recombination checks pass. Fresh VBDOS/PDS NBODY and QB ADDRM
outputs match originals. NBODY changes 4205 -> 4165 bytes on VBDOS and
2888 -> 2872 on PDS; modeled costs change 363752 -> 361952 and
360587 -> 359867 respectively. These are estimates, not hardware timings.
Artifacts: `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-signed-recombine-dbv_q48f`.

### Target-coverage checkpoint: FLAGS

A full `opportunity.py --targets` inventory finished with nonzero status:
missing references and provisional floating/event references still prevent
completion. It was one measurement run, not repeated runtime/test suites.
FLAGS was among the programs with no denominator despite constant-folded
branches. Its new complete hand listing in `docs/targets.md` keeps all twelve
numeric stores, six print calls and termination, costing 72+156+20=248.
The source values and store/output cost regression failed first without the
target. A focused three-compiler report now gives PDS 248/248=1.00x,
QB 296/248=1.19x and VBDOS 248/248=1.00x. Event-enabled variants remain
provisional under the existing policy. This changes measurement coverage,
not emitted code, and is not a claim that the overall goal is complete.

### Reuse an unchanged allocator spill reload

Complete machine-stage dumps now group every procedure into each phase file.
They exposed NBODY's repeated `mov si,[bp-28h]` at original address 0x23a:
SI still held the reload from 0x22a. The post-allocation peephole remembers
only allocator-owned frame reloads, within one basic block. Memory stores,
unknown effects, calls and constraints clear the proof; overlapping register
writes invalidate it, including partial registers and frame-base changes.
Loading the frame base itself never establishes a reusable address.

Before the update store: `mov si,[bp-28h]; mov [si+5ah],eax`.
After: `mov [si+5ah],eax`, using the unchanged SI. This is physical spill
cleanup, not a machine-dependent MIR optimization or a new LIR tier.
The real NBODY regression failed first. The 136 focused peephole checks
passed, followed by ten checks including the additional frame-base guard.
VBDOS NBODY is 4165 -> 4162 object bytes; PDS is 2872 -> 2869. Both outputs
match BC with timing calls intact. QB ADDRM matches at unchanged 857 bytes.
Artifacts: `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-reload-reuse-q57jswm5`.

### Fold untied spill sources while allocating

NBODY's signed division biases cannot be discarded merely because its source
describes squared distances: fixed-width squares and their sum can wrap, and
observed simulation ranges do not prove otherwise. Those corrections remain.

The allocator's spiller instead now folds one spilled, untied source into
ADD/SUB/AND/OR/XOR, at matching word or dword width. The destination must remain
in a register; groups, fixed requirements, clobbers and a second spilled
operand retain the existing reload path. This implements a spill decision
without creating a scratch interval. LLVM's local `InlineSpiller.cpp`,
`foldMemoryOperand` (around line 1034), likewise selects explicit untied uses.
It is not a new LIR optimization tier and adds no machine detail to MIR.

Before, at NBODY's first delta: `mov eax,[bp-2ch]; sub esi,eax`.
After: `sub esi,[bp-2ch]`. The Y delta receives the same change.
The real NBODY and five arithmetic-form regressions failed first. All 34
spiller checks pass; the preceding combined run also passed all 137 peephole
checks. An old test requiring a reload for ADD was replaced by the equivalent
reload assertion for the unhandled ADC form, with direct-source ADD covered
explicitly rather than preserving its old inefficient instruction shape.

VBDOS NBODY: 4162 -> 4156 object bytes; modeled cost 361352 -> 357352.
PDS NBODY: 2869 -> 2863 bytes; modeled cost 359267 -> 355267.
Both runtime outputs match BC, including all simulation coordinates and DONE;
timer readings are excluded from output equality. QB LNGMXX remains correct
at 838 bytes / 246 modeled units. These are estimates, not hardware timings.
Complete dumps, objects and runtime outputs:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-fold-spill-m06buebr`.

### Fold comparison spill sources and compose with accumulator spills

The spill-source fold now includes CMP without swapping operands or changing
its flag definition. A folded frame operand explicitly drops the original
instruction's relocation metadata: NBODY's promoted comparison at 0x106
otherwise tried to bind its former global fixup to a frame displacement and
refused emission. The real NBODY test failed before the comparison fold and
caught that refusal during implementation; both are now resolved.

Before the outer-loop test: `mov ebx,[bp-20h]; cmp eax,ebx`.
After: `cmp eax,[bp-20h]`. The inner body/other comparison similarly reads
its spilled index directly. VBDOS is 4156 -> 4151 object bytes and modeled
cost 357352 -> 355332; PDS is 2863 -> 2858 and 355267 -> 353247. Both runtime
outputs match BC. QB LNGMXX remains correct at 838 bytes / 246 units.

Folding also composes with the existing accumulator-spill path: when both
arithmetic operands are spilled, load the untied source and update the
accumulator slot in place, rather than loading both and storing one back.
No benefit from that case was observed in the selected pressure fixtures;
the measured gain above comes from comparisons. The old four-instruction
shape assertion is replaced by execution of the emitted spill operations,
checking accumulator value and unchanged source across five operations.
Forty spiller checks passed, followed by eleven focused checks including
word/dword comparisons with one or both operands spilled.
Artifacts: `/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-compare-spill-vx0gut06`.

### Rematerialize constants inside parallel copies

NBODY's zero value #70 had a single literal definition and five uses, all in
loop-initialization parallel copies. The spiller excluded every grouped use
from constant recognition, storing zero in a slot despite already knowing it.
It now excludes grouped destinations and unsupported grouped uses, but allows
an unconstrained, full-width scalar copy to take a known literal directly.
No instruction is inserted inside the group. A destination assigned by a
parallel copy is still excluded from constant recognition.

Before: `mov ax,0; mov [bp-1ch],ax`, later `mov bx,[bp-1ch]` and a
`push [bp-1ch]; pop [bp-26h]` frame copy. After: `mov bx,0` and
`mov word [bp-24h],0`; the source slot and its initialization disappear.
NBODY's synthetic frame reservation falls from 34 to 32 bytes. The initial
real-fixture regression failed first; 57 focused spiller/parallel-copy checks
pass. VBDOS NBODY is 4151 -> 4143 bytes and modeled cost 355332 -> 354726;
PDS is 2858 -> 2850 and 353247 -> 352641. Both runtime outputs match BC.
QB PRESSX remains correct at 1031 bytes / 627 modeled units.
Full before/after stage dumps and runtime output:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-group-remat-x_52mdd7`.

### Reject blindly strength-reducing NBODY's index shifts

At `2cc5159`, the remaining `other << 2` is deliberately excluded by
`strength._multiplies`: a cheap shift alone is not enough reason to introduce
another recurrence. A process-local experiment allowed just original address
0x110; no source rule or compiler default was changed. Full stage dumps show
the shift removed, but both the original index and the new offset remain:
the `other <> body` condition still observes the unscaled index.

Before latch: `inc bx` (plus the unchanged accumulator stores).
Forced after: `mov dx,[bp-30h]; inc dx; add bx,4; mov [bp-30h],dx`.
The extra recurrence displaces the original index into a spill slot and grows
the synthetic frame from 32 to 34 bytes. VBDOS NBODY grows 4143 -> 4159 bytes
and modeled cost 354726 -> 376926. This is a failed optimization experiment,
not a correctness-validated build and not hardware timing evidence.

Forcing only initialization's shift at 0x7e yields 4150 bytes / 354718 units:
seven extra bytes for eight modeled units. Neither experiment is adopted.
Future work here must account for the original counter's non-address uses,
not simply enable every shift candidate or put machine-cost rules into MIR.
Before/forced objects and every stage:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-shift-induction-96_jonay`.

### Numeric arithmetic arguments are not escaping addresses

CHAIN's seven remaining divisions led back to escape recognition, not
division selection. `push word [a]` carries a relocation identifying the
load's source; it does not pass a's address to B$DVI4/B$RMI4. Only numeric
PRINT arguments were previously excluded from the escape set. Consequently
later print calls appeared able to overwrite a/b, losing their constants.

The ABI now identifies the four established eight-byte long arithmetic
argument lists, alongside existing numeric PRINT arguments. Recognition
consumes the exact adjacent stack suffix and reuses the existing block-local
stack-frame tracker for nested calls and intervening non-stack instructions.
An outer argument below a nested call is not mistaken for that call's input.
Locally defined replacements, unknown callees and unestablished contracts
receive no numeric-argument exemption. No optimization pass gains ABI details.

Three CHAIN divisions fold away, e.g. `-1073741831 / 39678839` becomes
quotient -27 and remainder -2413178 instead of loads/CDQ/IDIV. Four divisions
remain because frame-temporary constants are independently invalidated by
print calls. The frame-memory guard has not been relaxed.

The three real-fixture regressions failed first. All 32 focused escape and
constant-call-memory checks pass. Runtime outputs match BC on all compilers:
PDS 1356 -> 1274 object bytes and modeled cost 869 -> 682; QB 1336 -> 1254
and 869 -> 682; VBDOS 1687 -> 1613 and 819 -> 642. These are ranking costs,
not hardware timings. Full before/after stage dumps and runtime output:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-numeric-escape-fz312fjx`.

### Close supported loop exits in SSA without machine overhead

LCSSA now inserts an exit phi for every loop-defined value read after a
single-edge dedicated exit. It runs before the loop transforms in each fixed-
point round, and IndVarSimplify follows the closure when replacing a redundant
counter and removes the obsolete exit phi. Multi-exit and shared-exit loops
remain unchanged pending
`LoopSimplify`; flags are never closed as data values.

The first HARR comparison exposed an over-split edge: phi elimination emitted
a copy and trampoline for a one-input exit phi. Such a phi is an identity, so
lowering now unifies its result with its input across semantics, uses,
definitions, fixed-register constraints and widths before allocation.

Before and after HARR VBDOS `/G3` assembly are identical: 40 instructions and
1038 object bytes. In particular, the loop exit remains `cmp ax,0Ah; jle
0060h; mov word ptr ds:[0],0Bh`, rather than the rejected intermediate
`jle 0060h; jmp trampoline; ...; trampoline: jmp exit`. Four LCSSA tests and
46 focused architecture, phi, induction and stage-dump checks pass. HARR and matrix on PDS, QB
and VBDOS, plus VBDOS NBODY, are byte-identical with LCSSA on and off. Stage
evidence: `/tmp/qbopt-lcssa-canonical.zC31cP`.

### MemorySSA foundation

`analysis/memoryssa.py` builds a conservative memory def-use graph directly
from MIR. Loads use the preceding state; stores, calls and barriers define
it. Join and loop phis connect states across control flow, including a loop
backedge to the procedure entry. Identity phis disappear. The graph must be
rebuilt after MIR changes; alias-aware clobber queries and pass consumers
are still pending.

Seven focused cases cover straight lines, diamonds, loop backedges, barriers,
calls without named cells, entry backedges and read-only loops. The missing
call dependency was observed failing before correction. Assembly before/after:
unchanged, because this increment adds an analysis with no pipeline consumer
and makes no transformation or emission change.

### MemorySSA clobber queries

The graph can now find the nearest possible writes to a queried memory cell,
walking every phi input and terminating on cyclic backedges. MIR alias facts
allow unrelated stores to be skipped. Partial overlaps, unknown writes, calls
and barriers remain clobbers; joins retain all possible definitions. A result
identifies memory state only: a forwarding pass must also prove scalar
availability and dominance. Precise call mod/ref remains pending.

Twelve focused graph/query tests cover these cases; the three initial query
tests failed before implementation. No pass consumes this analysis yet, so
before/after emitted assembly remains unchanged.

### First MemorySSA forwarding consumer

`avail.forwardable` now consults MemorySSA when its existing forward lattice
loses a store fact at a loop header. A unique reaching store may supply the
read only when it dominates the use, writes exactly the requested bytes and
does not cross the block-local stack boundary. Unrelated backedge stores
are skipped; aliasing writes and calls prevent the replacement. The existing
forward transform replaces the memory operand with an SSA value, leaving
allocation to the backend.

The preheader-store regression failed before the change. Four focused cases
cover successful replacement, aliasing backedges, calls and a bypass entry;
41 memory, forwarding and boundary checks pass. HARR VBDOS assembly is
identical with the consumer disabled/enabled (1038 object bytes); there is
no measured HARR speedup from this increment. All stages and both assembly
listings are in `/tmp/qbopt-memoryssa-harr-before` and
`/tmp/qbopt-memoryssa-harr`. For example, both retain:

```asm
mov es,[bx+2]
mov dx,2Ch
add dx,[bx+0Ah]
mov bx,dx
```

A broader transform check encountered the existing LNGMIX hoist assertion
`the fixture must actually move invariant work`; the exact test also fails
with the MemorySSA consumer disabled. Its investigation remains pending.

### Restore the hoist regression's active input

LNGMIX's divide/remainder already fold to 14285 and 5 before LICM; its only
hoist candidates are empty operations. The test therefore failed its setup
assertion, not phi preservation. It now uses HARR's descriptor work, which
actually moves, while retaining the cross-variable phi and movement assertions.
Injecting the original defect (reconstructing SSA after hoisting) fails with
`hoisting discarded the existing accumulator phi`; production code passes.
LNGMIX stage evidence: `/tmp/qbopt-lngmix-hoist`. This changes a test fixture
only; production assembly before/after is unchanged.

### Preserve raised call effects in MemorySSA

The clobber walker now uses a call's explicit MIR write effects, including
proven byte-range exclusions. A call without write metadata remains an
unknown clobber, and barriers still stop the walk. Runtime recognition stays
in the raise; the analysis introduces no helper-name or machine-specific rules.

A preheader value now survives a loop call whose write effects exclude the
entire cell. The regression failed before the change; a one-byte exclusion
for a two-byte read still prevents forwarding. All 42 focused MemorySSA and
boundary checks and 32 existing call-memory/escape checks pass.

CHAIN PDS before/after assembly is identical at 1274 object bytes: existing
passes already cover its cases, so no CHAIN speedup is claimed. Full stages
and assembly: `/tmp/qbopt-memoryssa-call-before` and
`/tmp/qbopt-memoryssa-call-after`. Both retain the initializer
`mov dword [0],40000007h` and the same four remaining frame divisions.

### Escaped pointer origins are not object extents

The shared MIR alias rule incorrectly treated a pointer escaping at one offset
as unable to reach another offset in the same object. A fail-first regression
demonstrated the resulting wrong transformation: a loop call receiving an
escaped origin at 0x1e allowed the word read at 0x20 to be forwarded from a
preheader store. That load now remains. Without extents, a nonempty escape set
for the segment cannot prove disjointness; explicit byte-range exclusions can.
The same rule now protects all consumers, removing constant propagation's
private workaround. Empty per-segment escape sets retain their precision.

42 focused alias, call-memory, numeric-escape and forwarding tests pass.
CHAIN PDS before/after assembly remains identical at 1274 object bytes;
stages are in `/tmp/qbopt-escape-extents-after`, compared with
`/tmp/qbopt-memoryssa-call-after`. Its four remaining frame divisions need
a separate frame-object escape proof; no frame preservation was assumed.

### Reuse dominating loads through MemorySSA

Forwarding now also recovers a prior load when the forward lattice loses it
at a loop header. The provider must dominate the read and name the same bytes;
the memory walk must reach its earlier state without crossing an aliasing
write on any path. Backedges are traversed explicitly. Block-local stack
addresses are still confined to their block. This extends value lifetimes
without choosing registers or introducing another optimization tier.

The preheader-load regression failed first. It now removes the loop memory
operand; aliasing writes, bypass entries and intervening loop stores retain
it. The focused gate passed 46 checks, followed by 13 graph checks including
the additional intervening-store case. HARR VBDOS assembly is identical with
this consumer disabled/enabled (1038 object bytes), so no HARR speedup is
claimed. Stage and assembly files: `/tmp/qbopt-load-reuse-before` and
`/tmp/qbopt-load-reuse-after`.

### Target-directed checkpoint after MemorySSA integration

At `7c09901`, the targeted VBDOS `/G3` measurements are:

| Program | Emitted cost | Reference | Ratio |
| --- | ---: | ---: | ---: |
| HOTLPX | 247 | 217 | 1.14x |
| PRESSX | 623 | 508 | 1.23x |
| LNGMXX | 230 | 208 | 1.11x |
| SPILL | 446 | 1122 | 0.40x |
| ROTATE | 274 | 294 | 0.93x |
| SEGLD | 7052 | 6704 | 1.05x |

These are model costs, not hardware timings, and cover only these six
configurations. They do not establish completion of the full matrix or
roadmap. FPCSEX costs 4452; its 1340 reference remains provisional because
it reassociates the sum and omits SINGLE conversions.

The current FPCSEX assembly (`/tmp/qbopt-fpcsex-current/s43-asm-emitted.txt`)
still loads a, computes a+b, multiplies by c and stores p; then reloads a,
recomputes a+b, divides by c and stores q. It retains all three SINGLE
stores and the final WAIT. No floating transformation was made at this
checkpoint: before/after arithmetic remains the listing in
`tools/references/README.md`. A useful next step must establish masked or
exception-free reuse, or reduce backend overhead without changing those
observable operations. Repeating optimizations on the six passing integer
targets does not address this remaining gap.

### Exact floating CSE across acyclic paths

Floating CSE can now reuse a dominating exact computation across a diamond
when every intervening operation on every path is proven exception-free.
Opaque operations and cycles still prevent reuse; memory reads retain their
independent alias guard. The current strict rounding and exception contract
is unchanged. This extends the existing proof rather than granting general
floating reassociation or speculative motion.

The new diamond regression fails with the former block-local restriction
and passes with the path proof. All 29 focused CSE and architecture checks
pass. FPDEEP VBDOS before/after assembly is identical at 1924 object bytes;
stage evidence is in `/tmp/qbopt-fp-path-before` and `/tmp/qbopt-fp-path-after`.
Runtime integer-derived floating bounds are still propagated only within a
block, which limits the new path to computations with available exact facts.
Extending that analysis over SSA is the next dependency for broader reuse.

### Propagate exact floating bounds over SSA

Floating integer bounds now follow definitions across blocks to a fixed point,
independent of block listing order. A phi joins bounds only after all incoming
values are bounded; cyclic recurrences without an independent proof stay
unknown. Calls do not mutate existing SSA values, but memory-load facts remain
specific to the load site and no floating-environment assumption crosses a call.
This enables exact cross-block CSE for runtime INTEGER-derived arithmetic.

The runtime-input and reversed-block-order tests failed before implementation.
All 32 focused SSA-path and architecture checks pass, including unknown phi
inputs and a growing cyclic recurrence. FPI2CS PDS assembly is identical with
the previous/current bounds analysis (1418 object bytes, three FILDs); dumps:
`/tmp/qbopt-fp-ssa-bounds-before` and `/tmp/qbopt-fp-ssa-bounds-after`.

The broader floatbounds file reports ten count-assertion failures. Two
representatives were reproduced with the previous bounds analysis: FPCALC PDS
has three conversions where the test expects one; QB FPDEEP has no indexed
loads where it expects three. Assertions remain unchanged. Other configurations
still need classification; no full floating regression-gate success is claimed.

### Conversion regressions count per runtime input

The nine FPICSE/FPI2CS/FPCALC conversion failures were stale static counts:
unrolling makes three READ sites, with one shared conversion per site. The
tests now require one FILD, one retained FST and one FSTP in each emitted
READ region. All nine configurations pass. Disabling CSE makes the PDS
FPI2CS case fail at two FILDs in one region, verifying that the test still
detects duplicate conversion work. Production assembly is unchanged:
FPI2CS PDS retains FILD at 0x004f, 0x00a2 and 0x00f0, one per READ.

QB FPDEEP is separately classified: r01-unroll expands five indexed loads
to fifteen; r02-cse has zero after constant-index folding. Its existing
count assertion remains unchanged pending an output-based replacement;
the absence of loads alone is not proof of correct numerical output.

### Validate QB FPDEEP constant-index folding

The QB FPDEEP baseline and optimized executables produce byte-identical
output, matching all eleven numeric golden results. The one-program `q-O`
end-to-end run passed; BC reported zero severe errors. Artifacts are in
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-fpdeep-output-8d_nepjg`.

The regression now checks all eleven constant numeric print arguments for
QB, requires LIR emission, and verifies that indexed floating loads are gone.
PDS and VBDOS retain their three-load/two-stack-copy assertions. This replaces
the obsolete QB load count with numerical evidence, not a blanket allowance
for missing loads. Production assembly before/after this test-only correction
is unchanged; the optimization was already present.

### Reuse owned floating reloads within a consumer block

Cross-region floating allocation now shares an owned extended-precision reload
between uses in one block. Calls, barriers, unknown instructions and explicit
stack operands end reuse. The allocator still preserves each destination's
rounded store and handles any increased register pressure with its existing
spill machinery. No MIR floating arithmetic is removed or reassociated.

The forked shared-value regression failed first with two reloads. The focused
allocation file passes 55 tests, including call/barrier boundaries. Its consumer
changes from `fld tword [owned]; fstp dword [out]; fld tword [owned]; fstp dword [out]`
to `fld tword [owned]; fst dword [out]; fstp dword [out]`. Both rounded stores
remain; the retained stack value is extended precision, not the rounded output.

FPCSEX PDS before/after assembly is identical. This closes an allocation gap
for shared values but does not yet improve that target. Complete stage dumps:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-float-reload-mpt9c8qd`.

### Unroll through LCSSA exit phis

The refreshed target report covered 78 existing p-g2/q-O/v-g3 configurations:
69 non-provisional results are within 1.5x; nine floating results remain
provisional. This is not the event/flag matrix or coverage of missing references.
FPDEEP exposed a concrete integration defect: PDS/VBDOS have two LCSSA exit
phis and the unroller rejected every exit phi. QB has none and already unrolled.

Unrolling now supplies final-iteration values on the new latch-to-exit edge,
while retaining initial values on the entry-test edge until branch folding
removes it. Phi inputs remain keyed to actual CFG predecessors. Recognition
and floating semantics are unchanged; the existing exact folding pass can
now see constant array indices on PDS/VBDOS too.

| FPDEEP | Modeled cost before | After | Object bytes before | After |
| --- | ---: | ---: | ---: | ---: |
| PDS /G2 | 12371 | 1777 | 1472 | 1838 |
| VBDOS /G3 | 12351 | 1777 | 1924 | 2291 |

Code grows from unrolling while modeled execution cost falls. The denominator
is still provisional; no target-completion or hardware timing claim is made.
For the first printed square, the emitted conversion changes as follows
(relocations named, intervening string output omitted):

```asm
; before                       ; after
fld dword [q]                  wait
fistp dword [bp-4]              push dword 144
wait                           call far B$PEI4
mov eax,[bp-4]
push eax
call far B$PEI4
```

Both production-shaped regressions failed before the fix. Six focused
unroll/FPDEEP checks pass, including FPCSEX refusing unprofitable expansion.
PDS and VBDOS end-to-end FPDEEP each pass all eleven numeric golden cases.
Before stages: `qbopt-fpdeep-unroll-9m59iys4`; after stages:
`qbopt-fpdeep-lcssa-after-cpfq2fn9`; execution artifacts:
`qbopt-fpdeep-p-g2-_s_c0hhf` and `qbopt-fpdeep-v-g3-dis9k6o7`, all beneath
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T`.

### Complete FPCSE's observable reference

The 98-unit output-only subtotal is no longer used as a complete FPCSE
reference. `docs/targets.md` derives the full source states: PDS/VBDOS require
five initial stores before the first FP checkpoint and four final stores;
QB checks before initialization and needs seven final stores. Exact dyadic
arithmetic cannot introduce subsequent FP exceptions, and no call occurs
inside the loop. All final globals and output calls remain observable.
The resulting independent costs are 157/145/157. Current emitted costs match
all three at 1.00x; no generated assembly changes in this instrument update.

Three renamed-object tests failed before implementation and now pass.
Eleven focused reference/event checks pass; unknown compiler and event-enabled
builds remain provisional. Compiler selection reads COMENT, not the filename.
FPCSEX, FPDEEP, missing references and the broader architecture checklist
remain unfinished. The final DOUBLE copy still needs the previously recorded
runtime DF/segment proof; its unresolved indirect-call analysis was not rerun.

### Global physical-copy elimination

The post-allocation peephole now intersects byte-level register equalities
over reachable CFG predecessors to a fixed point. A copy disappears only
when its source and destination already agree on every path. Partial writes
invalidate only affected lanes; conditional writes invalidate possibly changed
lanes. Unknown effects and calls discard the proof. The entry has no assumed
equalities, including when a loop returns to it. No machine fact enters MIR.

Before a diamond join: `mov ax,bx; je right; ...; join: mov ax,bx`.
After: the join's second MOV is absent when both arms preserve AX/BX.
Writing AH on either arm retains it; an AL=BL copy can still disappear.
Three fail-first cases now pass, with hazards and reversed block order covered.
The scoped backend run passes 151 tests; the remaining ADDRM LEA-shape failure
also fails with this pass disabled and was not weakened.

NBODY VBDOS before/after assembly is identical; no target improvement claimed.
Dumps are in
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-global-copies-sri89wqu`.
General operand substitution remains unfinished, so the roadmap item stays open.

### Native C reconstruction and selector rematerialization

`r_span.obj` no longer disappears at reconstruction.  Its unreferenced static
procedure at `0x034e` is admitted only after a bounded tentative walk from the
native `push bp / mov bp,sp` entry proves that every path returns and every
remaining byte in the containing gap is inert.  Applying the same rule to the
five available Borland C objects maps all of each code segment: `d_faces` has
2,050 instructions and four bodies, `pl_trace` 804 and three, `r_span` 1,808
and 27, `r_walk` 757 and five, and `sb_build` 502 and two.  Local near wrappers
which push CS before calling a far-returning procedure now distinguish physical
return-address depth from the callee's semantic argument cleanup.

Relocated selector immediates and ES-indexed static offsets now survive the MIR
boundary independently.  LOC_BASE fixups are no longer mistaken for numeric
zero, and an explicitly segmented generated operand uses OMF's target frame
rather than DGROUP.  With artifact-audited diagnostic contracts, `r_span.obj`
rebuilds from 8,734 to 8,235 bytes.  The emitted object has SHA-256
`44d9530cd670abe51658c0ac862dcff73baa744913038127a032f509661e3d00`; the
store reconstructed at `0x10ae` retains its OFF16 relocation to segment 4 with
addend `0x1800`, and its selector load retains the paired LOC_BASE relocation.
The UGL contract artifact is a real OMF library, so contract profiles now
validate a symbol against its archive member rather than accepting only a
standalone OBJ.  `MATHC.LIB`, which defines `F_FTOL@`, is not available on this
host; therefore no complete persistent profile was fabricated.

The corrected selector SSA initially exposed a backend regression in
`pl_trace.obj`: current output was 2,015 bytes with ten spill bytes, versus the
previous accepted 1,969-byte artifact with four.  Adjacent stage dumps showed
three two-byte slots storing ES selector values.  MIR CSE had correctly kept
one load of `[bp+1e]` across intervening `LES` definitions; generic allocation
then tried to keep overlapping selector values in the one physical ES resource.
The backend now recognizes a unique, unchanged positive BP-relative selector
load as a rematerialization recipe, retries allocation after cheap
rematerializations before spilling the other conflicts, and forwards identical
physical reloads through read-only x87 operations.  It invalidates that fact on
an actual segment write or a possibly aliasing store.

The resulting helper has the hand-derived sequence: one `mov es,[bp+1e]` before
each of the three runs which need that selector, no selector stores, and no
repeat reload between adjacent ES-relative x87 reads.  Its frame is again the
original 34 bytes plus exactly four spill bytes, and its final instruction count
is 541 versus 542 in the older artifact.  The whole object is 1,985 bytes.  The
remaining 16-byte object-size difference is outside spill expansion and follows
earlier semantic reconstruction changes; no equivalence claim is made from size
alone.  Stage evidence is in `/tmp/qbopt-pltrace-remat`.

Per the user's explicit instruction for this checkpoint, no tests were written
or run.  Only syntax compilation, selected-file diff checks, stage dumps, object
rebuilds, disassembly, hashes, and parsed relocation records were used.  The
project-wide target and the regression-test gate remain incomplete.

### Composable native-call evidence and R_WALK allocation baseline

External-call profiles now compose without copying independent audits into a
new JSON file.  Repeating `--contracts` loads each hash-checked profile against
the common artifact root, rejects a symbol declared by more than one profile,
sorts the resulting rules, and records one argument-order-independent
fingerprint.  A single profile retains its existing fingerprint.  Composing
the current generated qrender interfaces with the UGLV audit yields 201 unique
contracts and fingerprint
`876e9244ca162f734f1a53488f32574959efd2092c1f15a6c7592c3ae2714984` in
either argument order.

`tools/contracts.py` could not initially audit `r_sweep.obj`: its shared module
reader treated every standalone OMF object as an empty library.  The reader now
uses the archive layout for a LIB and parses exactly one valid THEADR module for
an OBJ.  That exposed a second measurement defect: generic register-copy
handling shadowed the analyzer's `mov bp,sp` case, and fixed stack reservation
plus `leave` were unmodeled.  With frame moves ordered first and signed
`add/sub sp,imm` plus `leave` modeled, the analyzer follows both near calls to
the local helper and derives the public routine's only normal return as
`cleanup: 18`, with no unknown path.  Raw bytes at `0x0135` are `CA 12 00`
(`retf 18`).  The audit is pinned to `r_sweep.obj` SHA-256
`01073a08dff883e30870a9ec959bd2f79a66ffa8c065191d9fd0db77d4c6499c` in
`docs/contracts/qrender-r-sweep.json`; UGL cleanup-only facts are independently
pinned to the 138-member UGLV archive in `qrender-uglv.json`.

The three available profile sets cover `R_SWEEP_ROW`, `UGLDCSIZE`,
`UGLSETVIEW`, `UGLSPANBEGIN`, and `UGLSPANTP` for `r_span.obj`.  Production
still refuses the original object unchanged at native procedure `0x04d5`.  Its
two remaining unknown calls are `F_FTOL@` at `0x0cec` and `0x0d15`; raw caller
assembly loads the operand into x87 `st0` and pushes no stack argument, but the
defining Borland MATHC library is absent, so no persistent ABI contract was
made from caller convention alone.  The earlier `F_FTOL@` call at `0x047d` has
the same shape in another procedure.  `MATHC.LIB` and `CL.LIB` are absent from
all available work and temporary trees.  Consequently `d_faces` still lacks
`F_FTOL@` plus three `R_SPAN_*` and `SB_BUILD` contracts; `sb_build` lacks
`F_FTOL@` and `F_SCOPY@`; `r_span` lacks only `F_FTOL@`.  Unknown callees remain
unknown rather than being admitted from historical hashes whose bytes cannot
be checked here.

`r_walk.obj` has a complete available contract set and rebuilds through the
production LIR emitter to 2,112 bytes, versus the earlier recorded 2,130-byte
artifact.  Its five procedures have these adjacent backend counts:

| Stage | Procedure counts |
| --- | --- |
| lowered | 69, 10, 357, 28, 267 |
| phi elimination | 69, 12, 361, 28, 326 |
| two-address | 70, 12, 366, 28, 360 |
| coalesce | 70, 10, 366, 28, 346 |
| register allocation | 71, 10, 385, 28, 309 |
| prologue | 71, 10, 388, 28, 314 |
| peephole | 70, 10, 384, 28, 311 |

The third procedure's allocation growth is concrete rather than inferred from
object size.  For example, values loaded through ES are saved in new spill
slots such as `[bp-22h]`, immediately reloaded into the same physical register,
then stored into existing program locals; those spill homes have later reads,
so deleting the first round trip alone would be wrong.  Reusing the already
written stable local as a spill/rematerialization home is the next general
backend question.  Complete dumps are in `/tmp/qbopt-rwalk-current`.

That backend question is now answered conservatively in LIR.  A spilled value
may use a source-program frame local as its home only when one original
negative BP-relative store dominates every other use, no use is part of a
parallel copy, and the whole procedure contains no call, incomplete memory
barrier, or store which may alias that cell.  The initializing store remains;
later uses get short reload values from the existing local before coloring.
This is backend rematerialization and introduces no physical register fact into
MIR.

On the real `r_walk.obj`, the rule fires for the fields copied to `[bp-6]` and
`[bp-0Ah]`.  Register-allocation output for the third procedure drops from 385
to 381 instructions, its extra spill reservation from ten to six bytes, and
the complete emitted object from 2,112 to 2,100 bytes.  Before, the first field
included `mov [bp-22h],bx; mov bx,[bp-22h]; mov [bp-6],bx`; after, it is directly
`mov [bp-6],bx`, and later consumers load `[bp-6]`.  The analogous long-lived
value reached through a separate copy is not admitted: it has no pre-existing
frame home, and the coalescer deliberately keeps its short load range separate
because merging it fails the colorability guard.  Post-change dumps are in
`/tmp/qbopt-rwalk-framehomes2`.  `pl_trace.obj` remains a 1,985-byte LIR rebuild,
so selector rematerialization is unchanged by this rule.

This checkpoint again contains no authored or executed tests at the user's
request.  Syntax compilation, profile validation, real-object stage dumping,
raw disassembly, hash checks, and selected-file whitespace checks succeeded.
The required fail-first regressions and project-wide integration gate remain
open while that restriction is in force.

## 2026-09-12: a computed store does not name what it stored

`avail.stored_from()` answered "this cell now holds value V" for any operation
with one store, no load, and exactly one value read.  `inc [x]`, `dec [x]` and
`add [x],k` fit that shape and store something other than what they read; the
deedlines rewrite contains 543 of them.

The availability lattice then carried the pre-store value across the store, all
three predecessors of the join agreed on it, and `forward` served the reload of
`kxy0%` from the value the variable held before the branch that changed it.
zoomdistort drew 5,235 wrong screen bytes.

Copy propagation inside `cse` only exposed it.  Until both arms of the branch
read the same SSA value the meet disagreed and the wrong fact never reached the
join, which is why the failure looked like a CSE bug and bisected into one.

`stored_from` now requires `Kind.STORE` or `Kind.ARG` -- the two shapes that put
a value in a cell unchanged.  Emitted code for the update, before and after:

    mov [bp-8Ah],bx          mov [bp-8Ah],ax
    ...                      mov ax,[bp-8Ah]
    cmp ax,0FF9Ch            cmp ax,0FF9Ch

The reload costs two instructions on two paths.  deedlines mark 3 goes from
5,235 differing bytes to none; marks 1, 2, 4, 6, 7 and 8 stay byte-identical.
actions3d (mark 5) still differs by 3,067 bytes and is not this bug.

## 2026-09-12: the emulator's segment protocol is not a dialect's

Under /FPi the compiler emits `int 3Ch` for a segment override, with the real
ESC opcode after it.  `blocks.decoded_instruction` restored the ES that
interrupt stands for only when the module's family was `vbdos`, on the strength
of a patch read out of VBDCL10E.

So in QuickBASIC objects every emulated float access through a dynamic array
decoded as `fld dword [bx]` -- DS, not ES.  The reference carried no segment
value, the `mov es,[si+2]` before it had no reader, and `dead` removed it.
deedlines' translate3d then divided by whatever ES held, every projected point
landed outside output3d's range test, and actions3d drew an empty screen.

Read back out of the running program, which is the only place the answer is:

    0824:A3F2  90 26 D9 07      (was CD 3C D9 07)

NOP plus a 26h ES override, in QuickBASIC 4.5's own deedlines.  The protocol is
the emulator's; the family test is gone.  The raised load now reads

    fld  [es:bx+0x0]   base=v4_20  segment=v6_1   uses=(v4_20, v6_1)

Emission already handled this: `_emulator_protocol` takes the interrupt byte
from the original site and `fpu.wrapped` accepts either a bare or 26h-prefixed
ESC under the 3Ch protocol.

## 2026-09-12: a store forgets nothing a register holds

`peephole.reloads()` cleared its whole table at any store and kept one cell
per register. Both are wrong. A store changes no register's value, and two
bp-relative slots whose byte ranges do not meet cannot be the same byte --
arithmetic, not analysis. A register also holds every slot it has been read
out of or written into since, not the last one.

It now invalidates only overlapping slots and remembers each register's cells.
`spiller._store` marks its own stores so an inserted one can say it writes that
slot and nothing else; it carries the `op` of whatever it stands beside, whose
stores are not its own.

deedlines, every mark still byte-identical to BC's output:

| mark | before | after |
| --- | --- | --- |
| 1 | 1.21x | 1.28x |
| 3 zoomdistort | 0.98x | 1.00x |
| 4 rgblights | 0.99x | 1.02x |
| 5 actions3d | 1.24x | 1.26x |
| 7 plasmablobs | 0.92x | 0.93x |
| total | 1.08x | 1.088x |

qbdemo's shadebob went 1.18x to 1.28x; oimad is unchanged at 1.01x.

## 2026-09-12: more alias precision, and a measurement that could not answer

`may_alias` answers True for any indexed operand against anything in another
space, so one `POKE` through `es:bx` makes every frame local in the loop
aliased. Indexing moves within the thing indexed, and Space.FAR is a $DYNAMIC
array element in the far heap, so the rule that already separates a frame local
from a named variable arguably separates it from both. Relaxing it let
`forward` serve plasmablobs' reload of `x%`.

plasmablobs grew from 1,079 instructions to 1,092, so the change was reverted:
the longer live ranges a more precise analysis exposes are spilled rather than
held, and the gain arrives as extra spill slots and reloads.

The first version of this entry also claimed five other procedures grew. That
was the instrument, not the subject: the variant objects were built with MIR
passes restricted to plasmablobs, so every other procedure was being compared
against its own unoptimised form. Only the plasmablobs figure was a comparison
of like with like. Whether the relaxed rule helps or hurts elsewhere is still
unmeasured.

## 2026-09-12: the relaxed alias rule, measured whole-module

The entry above left the question open. Built with the whole module optimised,
the relaxation costs instructions everywhere it changes anything:

| | now | relaxed |
| --- | --- | --- |
| plasmablobs | 1078 | 1092 |
| zoomdistort | 728 | 753 |
| spheremaplasma | 1080 | 1108 |
| all 11 bodies | 6862 | 6932 |

Reverted for good.

## 2026-09-12: a reload is redundant on every path, not just the one behind it

`spillforward` compared a block's first instruction with its predecessor's
last, and `peephole.reloads()` did the same thing within one block. Neither can
see plasmablobs' x loop, which reloads its counter at the top although the
latch left it in `ax` -- the edge between them is a block holding one `jmp`.

Both are now one availability fixpoint over (register, slot) facts in
`spillforward`, met at every incoming edge; `reloads()` is gone. Two things had
to be right for it to find anything:

  - A `jmp` writes nothing. `_register_effects` answers only for instructions
    that fall through, so reading its `None` as "gives up" ended every fact at
    the end of every block that ends in one.
  - `op.stores` is the last word only for an instruction that is its own op. A
    reload carries the op of whatever it stands beside, stores and all, so
    reading it as a write to memory ended every fact at the very instruction
    the facts were there to answer.

## 2026-09-12: what is dead after a block is not a question a block can answer

`peephole.overwritten()` walks a block backwards from "assume everything live",
which is exactly where a phi's parallel copy is written. `backend/liveness.py`
computes the lanes dead on exit from each block and seeds it.

A call is not a register barrier either. `requires` and `clobbers` are the
contract the allocation is already built on -- `allocate` keeps live ranges in
registers a call does not clobber -- so liveness reads a call the same way
instead of treating every register as live across it.

## 2026-09-12: a loop counter does not need a second home

`spiller._frame_homes` refused any body containing a call and any value with
more than one definition, so deedlines' plasmablobs stored `x%` to a slot of
its own beside its own `[bp-2Eh]`, once per iteration of a 64000-iteration
loop.

Both restrictions asked a question about the body that is really about a point
in the program: whether the store is the last thing to have written the cell.
It is now a forward dataflow -- the store establishes it, a definition of the
value, a call, an incomplete barrier or an aliasing write ends it, and every
use must have it. A call before the loop is no longer a reason: the store
re-establishes the fact on every path into the loop.

`[bp-0A2h]` is gone from plasmablobs. Its x loop is 77 instructions against
BC's 76; the one left is a phi copy on the backedge for a value read after
both loops, which the allocator pays rather than hold a register across them.

All three demos, every screen byte-identical to BC's (qbdemo's mark 1 keeps its
known 9-byte fractal difference):

| | before | after |
| --- | --- | --- |
| deedlines total | 1.092x | 1.102x |
| deedlines 3 zoomdistort | 1.00x | 1.01x |
| deedlines 4 rgblights | 1.06x | 1.07x |
| deedlines 7 plasmablobs | 0.95x | 0.99x |
| oimad | 1.01x | 1.01x |

qbdemo is 1.32x, 1.22x and 1.32x on marks 1, 3 and 4. Its mark 2 is not an
instrument at this resolution: five runs of one binary spanned 824k to 942k
ticks, so nothing under about 15% can be read from it.

## 2026-09-12: the alias relaxation again, with the allocation defects fixed

The relaxation above was worth re-measuring once the redundant reloads, the
duplicate home and the block-local liveness were gone -- its cost looked like
an allocation failure, not an analysis error.

It is not. On the same eleven bodies: 6841 instructions without, 6913 with
(plasmablobs +16, zoomdistort +26, spheremaplasma +26). The extra precision
makes three more loads loop-invariant, `hoist` moves them out, and the values
are then live across the whole loop, so the allocator spills each one back to
where the load was and pays the store as well.

The spill weight is already LLVM's -- references weighted by loop depth over
live length -- and that is the formula that decides this: a value defined in
the preheader and used inside the loop has a denominator the length of the
loop, so it is always the cheapest thing to spill. Nothing short of splitting
the live range changes that answer, which is what LLVM does instead of
spilling whole ranges.

## 2026-09-12: the program's own redundant loads

`spillforward` removed only allocator-owned reloads. A program load of a local
into a register that already holds it is as redundant, and the availability
fact proves the same thing about both. Six more instructions across deedlines;
all three demos unchanged.

cycleblobs' latch keeps its reload of `x%` even so: its inner loop writes the
screen through `es:bx`, which no sound rule here separates from a frame local.

## 2026-09-12: a rebuilt value is not a slot, and a read is not a store

Two things the spiller knew about slot-backed values and not about the ones it
rebuilds.

**Rebuilding a load.** `_frame_loads` recognised only word selectors out of a
positive bp-relative argument slot. `_stable_loads` asks the general question:
a value whose one definition is a load, from a cell nothing changes on any
path to any use, is that load. Reading the cell again is the same value, so
the load goes back where it was and no slot is taken -- which is what makes a
hoist free when the allocator then refuses the value a register.

**Folding the read.** `_source` folds a spilled value's use into the
instruction that wanted it, and it ran only on slot-backed values. A rebuilt
cell reads exactly as well, so the rebuild was costing a whole instruction
more than the spill it replaced -- `mov si,[bp-2Eh]; add dx,si` where BC
writes `add dx,[bp-2Eh]`. Only the read: `_in_place` and `_tied` write the
cell back, and a program's own variable is not there for that.

plasmablobs' x loop is now 76 instructions, which is BC's own count.

| mark | before | after |
| --- | --- | --- |
| 3 zoomdistort | 1.01x | 1.03x |
| 4 rgblights | 1.07x | 1.08x |
| 7 plasmablobs | 0.99x | **1.00x** |
| deedlines total | 1.102x | **1.106x** |

qbdemo 1.32/1.28/1.22/1.31 and oimad 1.01x, both unchanged; every screen in
all three byte-identical to BC's but qbdemo mark 1's known 9 bytes. Only
cycleblobs is still under parity, at 0.99x.

## 2026-09-12: pricing a rebuilt value lower does not pay

If the spiller can rebuild a value for free, the allocator should prefer to
spill it -- so its weight was scaled down, at 0.5 as LLVM does and at 0.01.
Both moved instructions from one loop to another: zoomdistort lost eight and
plasmablobs' x loop gained two, because the saving is only the store, and a
hoisted load's store is in the preheader where it costs almost nothing. The
three loads per iteration that keeping it in a register avoids are worth more
than the one spill it displaces. Reverted; the honest version prices each
reference by what spilling it would actually emit, which is a bigger change
than a factor.

## 2026-09-12: which half of the alias relaxation costs the instructions

Measured apart rather than together, the two halves are not alike. Against
6831 without either:

| | total |
| --- | --- |
| FRAME disjoint from indexed FAR only | 6882 |
| FRAME disjoint from indexed SEGMENT only | 6834 |

The FAR half carries all of it -- plasmablobs 1062 to 1079, zoomdistort 723 to
740, spheremaplasma 1079 to 1098, the same damage the full relaxation did. The
SEGMENT half is a wash.

Neither unblocks cycleblobs' latch reload, which was the reason for trying:
the screen write its inner loop makes is neither, so the reload stays whatever
this rule says.

## 2026-09-12: the FAR rule, priced in hot loops instead of body totals

The screen write cycleblobs' inner loop makes is `Space.FAR` -- a $DYNAMIC
array element in the runtime's far heap -- and that is what ends the fact that
`bx` already holds `x%`, one instruction before the loop reloads it. Making
FRAME and indexed FAR disjoint in `may_alias` removes that reload and brings
the loop to 62 instructions against BC's 63.

It also takes plasmablobs' x loop from 76 -- BC's own count -- to 81, and the
five are three new spill slots reloaded inside the loop:

    mov bx,[bp-0A2h]      mov si,[bp-0A4h]      mov si,[bp-0A0h]
    mov bx,[bx]           add bx,[si]           mov cx,[si]

The precision makes three address computations loop-invariant, `hoist` moves
them out, and all three are spilled straight back -- so each use is a reload
and an indirect load where it had been the arithmetic. Weighted by the marks
they are in, 5 out of 76 in a 12.8M-tick loop against 1 out of 63 in a
10.5M-tick one, it loses.

The rule is not wrong; the pipeline cannot hold what it exposes. What is
missing is rematerializing cheap address arithmetic the way `_stable_loads`
now rematerializes a load -- LLVM's `isAsCheapAsAMove`. Until that exists this
stays reverted, and this is the third and most precise measurement saying so.

## 2026-09-12: a copy for what comes after the loop belongs after the loop

A phi whose value is computed in a loop and read after it becomes a copy on
the way back to the header, so it runs every iteration to hand over a value
nothing inside looks at. plasmablobs ended its inner loop with `mov di,bx` for
a call after both loops -- 64000 copies for one use -- and cycleblobs' inner
loop carried two.

`backend/copysink.py` moves such a copy to the exit block. Two things had to
be asked correctly:

  - **Whose liveness.** The destination is live-in at the block that branches
    out of the loop, because the *exit* reads it -- which is the path the sunk
    copy is for. The question is whether it is dead on the way back to the
    header, at that block's in-loop successors.
  - **Which blocks are in between.** The copy is in the block before the
    branch, and the loop body writes its source. Only a path from the copy to
    the exit that does not come back through the copy's own block can be why
    the last copy before the exit is wrong, so the walk leaves that block out.

plasmablobs' x loop is 75 instructions and cycleblobs' inner loop 62, against
BC's 76 and 63. Every screen in all three demos byte-identical but qbdemo mark
1's known 9 bytes.

| mark | before | after |
| --- | --- | --- |
| 2 spheremaplasma | 1.02x | 1.06x |
| 6 cycleblobs | 0.99x | **1.00x** |
| 7 plasmablobs | 1.00x | 1.03x |
| deedlines total | 1.106x | **1.114x** |

qbdemo mark 3 went 1.22x to 1.24x; oimad is 1.01x. No mark of any demo is
below parity with BC any more.

## 2026-09-12: a sign word through the stack, and two instruments

Re-scoring `--targets` over the committed PDS `/G2` fixtures found two things.

**The bounds fixtures were never comparable.** `_program` reads the THEADR, so
`harr-bounds-p-g2` -- a `/D` build of the same source -- was scored against
HARR's plain-build target and read as a 16.67x miss (30,578 against 1,834);
arridx-bounds read as 6.20x. A `/D` build tracks the line at every statement
and checks every subscript. `opportunity.py` now says so, the way it already
does for an event-enabled build. Those two targets are uncovered, not passed.

**A divide's sign word went through the stack.** The raise decodes BC's `cwd`
faithfully as `extract(sign_extend(x), 16)`, and lowering had one rule for an
extract: push the dword and pop both halves. stride's loop paid

    mov cx,5 / movsx eax,bx / push eax / pop ax / pop dx / mov ax,bx / idiv cx

where the popped low half was dead -- the divide reads the word itself -- and
the sign is one `cwd`. `_word_division` already emits exactly that Semantics;
`_extract` now does too, and an extract of the low half is a `mov`.

Only where a divide asked for it. `cwd` pins its word to ax and takes dx,
which is free where `idiv` wanted dx:ax and a shuffle anywhere else: emitting
it for every sign word cost deedlines' actions3d 1.5% of its running time
(mark 5 1.26x to 1.24x) while shrinking the object. Gated on the extract being
a divide's high half over the word beside it, actions3d is back to 1.26x.

stride 3.10x to 2.54x, every other row in the suite unchanged. Six documented
targets are still above 1.5x: stride 2.54x, jumps 2.95x, hotlpx 2.06x, harr
1.77x, lngmxx 1.57x, segld 1.56x. The `-x` rows want the native-FPU
configuration this scan does not use.

Demos, every screen byte-identical but qbdemo mark 1's known 9 bytes:

| mark | before | after |
| --- | --- | --- |
| 1 prehistoricode | 1.29x | 1.30x |
| 4 rgblights | 1.08x | 1.10x |
| 8 telos | 1.09x | 1.10x |
| deedlines total | 1.114x | **1.115x** |

qbdemo 1.32/1.25/1.24/1.29 and oimad 1.01x.

## The sign-extended dividend, and what the inner loops have in common

**A dividend that reached a long by `movsx` is still one `idiv r16`.**
`raising_division` collapsed a three-argument divide only where BC wrote
`cwd`. stride's dividend comes from `movsx eax,bx`, which the raise says as
`extract(sign_extend(x), 16)` -- the same fact in the shape the long raise
leaves. It now matches both, and the `op.merges` refusal became a rewrite:
`idiv r16` ties dx:ax to the pair it writes, and dropping the high half drops
its tie with it, because the lowering builds a fresh high from `cwd`.

With the divide a `divmod`, `induction._quotients` recognises `i \ 5` where
`i` strides by five, and stride's loop is the sequence 0, 1, 2, ... : 13
instructions to 5, and the sum folds to a constant. stride 2.54x to **0.76x**.
Five documented targets remain above 1.5x.

**The spiller asked select.py too late.** Relaxing the merges rule let a tie
land on a widening multiply, and `_tied` wrote `imul [bp-4Ah],cx` -- a form
x86 does not have. Its docstring said an unsupported form "refuses there,
which is a refusal and not a wrong program"; a refusal is what qbdemo became.
`_tied` now asks select.py whether the form exists before committing to it.
Neutral on the demos, four bytes on segld.

**Two instrument defects.** `BenchSnap` masked IRQ0 across its reading, which
stops the BIOS tick from moving without making it right: a tick already
pending is not counted. And mark 4 spans about five BIOS ticks, so one tick of
inconsistency is 17% of the reading -- qbdemo mark 4 read 1.29x and 1.06x for
two programs whose mark-4 code is the same instructions, and flipped sign
between the dynamic and the normal core. The mark is too short to read below
about 20%; the normal core is what settles a question that size.

### What the inner loops have in common

Three demos, one shape. qbdemo's shadebob inner loop:

    mov di,[bp-76h] / mov es,[0] / mov dl,[es:di] / and dx,0FFh
    mov es,[bp-96h] / mov [es:si],dx / inc di / mov [bp-76h],di
    inc cx / add si,2 / mov [bp-7Ah],cx / cmp cx,9Fh / jle

and oimad's palette store, which recomputes the descriptor as well:

    mov bx,[bp-48h] / lea cx,[ebx+ebx] / lea si,[bp-1Ch] / mov di,cx
    add di,[si+0Ah] / mov es,[si+2] / mov [es:di],ax / inc bx
    mov [bp-48h],bx / cmp bx,2FFh / jle

Four instructions of work in eleven to thirteen. Over qbdemo's seventeen
innermost loops: 9.3% reloads of a local, 8.2% stores back to one, 6.5%
segment loads, 3.1% loop-invariant address arithmetic. Twenty-seven per cent
of innermost-loop instructions move values that never needed to leave a
register.

Two mechanisms, and they are the same problem in an order.

**A far store is assumed to alias the frame.** Every one of these loops writes
through `es:`, and `may_alias` short-circuits an indexed operand to True. So
`mov [es:di],dx` may clobber `[bp-76h]` and the descriptor at `[bp-1Ch]`: the
counter goes back to its slot every iteration and the array's base and segment
loads are pinned inside the loop. The counter is both reloaded and stored --
not a spill, a fence.

**Segment registers are not allocated.** `Register.FS` and `Register.GS`
appear in one line of the codebase, `target.SEGMENTS`, as scenery. A loop
reading one segment and writing another reloads `es` twice an iteration and
cannot hoist either, because there is one register for two values.

This is why the alias relaxation lost four times: relaxed, the segment loads
become loop-invariant, and there is nowhere to hoist them to.

**Corrected, by reading the LIR's own flags rather than emitted addresses.**
The counting above is of emitted text, and it cannot tell an allocator spill
from the program's own memory. Asked of `spill_reload` and `spill_store`:

| | qbdemo, 32 innermost loops | deedlines, 183 |
| --- | --- | --- |
| the program's own | 75.3% | 79.3% |
| inserted register-to-register moves | 5.1% | 6.6% |
| allocator spills | 4.4% | 3.9% |
| inserted constant materialization | 4.1% | 2.5% |

So most of that traffic is not spilling, and the average hides where the
spilling is. PLASMA's `loop@0xe2d` -- qbdemo mark 3, two thirds of the demo's
running time -- is 18 spills in 74 instructions, and its latch is sixteen
instructions to advance five counters:

    BX := mov [[bp-0x96]] ; SPILL RELOAD / BX := add BX, 0xb
    CX := mov [[bp-0xa6]] ; SPILL RELOAD / CX := add CX, 0x3
    DX := mov [[bp-0x98]] ; SPILL RELOAD / DX := add DX, 0x2
    SI := mov [[bp-0x9a]] ; SPILL RELOAD / SI := add SI, 0x4
    DI := mov [[bp-0x9c]] ; SPILL RELOAD / DI := add DI, 0x9
    push [[bp-0x9e]] / [[bp-0x94]] := pop / five stores back

Those five are strength reduction's derived counters, one per `sine(...)`
argument. Spilled, each costs reload, add and store where recomputing the
argument is `mov bx,ax; add bx,[f]` -- two instructions, and nothing carried
across the backedge. The transform is a net loss at this site and the
allocator cannot undo it.

`strength.py` says the assumption outright: "Allocation owns pressure and
spilling." It cannot, when the pass ahead of it has committed five
loop-carried values to a six-register file. LLVM's LSR prices formulas
against register pressure for this reason; this one does not price at all.

Order, by where the evidence is: price strength reduction's loop-carried
pressure, then segment allocation, then FRAME-vs-FAR. The alias relaxation
alone re-runs an experiment whose answer is already recorded.

One correctness lead sits in the same territory: a `BenchSnap` that compared a
LONG reference parameter it had written earlier in the same loop spun forever
in the qbopt build where BC's build ran. Worked around in the harness, not
diagnosed.

Measured after the batch, every screen byte-identical but qbdemo mark 1's
known nine bytes:

| demo | marks | total |
| --- | --- | --- |
| qbdemo | 1.38 / 1.25 / 1.22 / 1.06 | 1.25x |
| deedlines | 1.23 / 1.06 / 1.03 / 1.09 / 1.26 / 1.00 / 1.03 / 1.10 | 1.11x |
| oimad | 1.02 | 1.02x |

qbdemo mark 4 reads 1.09x under the normal core against 1.06x under the
dynamic one, which is the whole width of what that mark can say.

## Strength reduction, priced

The pass invented counters without counting them. Each one is a value live
around the whole loop, so a loop given more than the target has registers
gets every one of them spilled, and a spilled counter costs reload, add and
store where recomputing the expression it replaced costs two instructions and
carries nothing. qbdemo's plasma nest -- mark 3, two thirds of that demo --
had five, and its latch was sixteen instructions to advance them:

    BX := mov [[bp-0x96]] ; SPILL RELOAD / BX := add BX, 0xb
    CX := mov [[bp-0xa6]] ; SPILL RELOAD / CX := add CX, 0x3
    DX := mov [[bp-0x98]] ; SPILL RELOAD / DX := add DX, 0x2
    SI := mov [[bp-0x9a]] ; SPILL RELOAD / SI := add SI, 0x4
    DI := mov [[bp-0x9c]] ; SPILL RELOAD / DI := add DI, 0x9
    push [[bp-0x9e]] / [[bp-0x94]] := pop / five stores back

`Where.registers` is the budget, told to the pass rather than asked of the
machine -- a number is not machine form, and zero means nothing was said.
`pressure()` moved from `legacy/regalloc.py` to `analysis/liveness.py`, which
is the rule-5 example liveness.py's own docstring already gives, and gained
an `inside` so it can be asked of one loop.

**Two models were wrong before the third worked.** Pricing against peak
pressure inside the loop refused the array-indexing reductions the pass
exists for -- peak is high exactly where the multiply chain still is -- and
cost harr 1.77x to 5.28x, matrix 1.03x to 1.63x, segld 1.56x to 3.13x.
Pricing against values live across the backedge is no better: a promoted loop
legitimately carries more than the register file, and harr's loops report
nine against six registers.

What works is the count of recurrences the loop already drives, `_RESERVE`
for what its body computes with. Per round, not per call: a derived counter
is a phi advanced by a constant, so the next round reads it as a recurrence
and the budget shrinks as the pass spends it. Counting only one call's
additions let five rounds add five counters, one each.

`_RESERVE` swept against the suite and qbdemo's loops: 1 and 2 leave the
suite untouched, 3 costs harr 182 instructions and nested 8. Two is the knee.
Sorting candidates to spend the budget on multiplies first was measured and
dropped -- it cost harr 40 instructions and made plasma's loop worse.

plasma's inner loop 18 spills in 74 instructions to **12 in 68**; its latch
sixteen instructions to three. Every row of the documented suite unchanged.

| demo | before | after |
| --- | --- | --- |
| qbdemo mark 3 | 1.22x | 1.23x |
| qbdemo mark 4 | 1.06x | 1.13x |
| qbdemo total | 1.25x | **1.26x** |
| deedlines total | 1.11x | 1.11x |
| oimad | 1.02x | 1.02x |

The loop is dominated by its array loads and the FP emulator, not its latch,
so thirteen instructions out of a hot loop is two tenths of a per cent of
mark 3. The mechanism is right and the site was not where the time is.

## qbdemo mark 1's nine bytes, narrowed

Every one is at screen x=160, rows 98-105, and qbopt writes 1 where BC writes
2, 3, 155, 255, 255, 4, 2, 2, 2. Through `render`'s scaling that column is the
fractal's centre, which is the midpoint pixel `fracline` computes on its own
after the scanline loop -- with `y` left over from a mid-body `EXIT DO`, which
made `loopexit.evaluated` the obvious suspect.

It is not. Three results, each a build and a run:

- **Not an optimization.** With `fold, decide, dead, segments, hoist, forward,
  drop_loads, drop_stores, promote, strength, unroll` all off -- qbdemo down
  to 1.14x from 1.26x -- mark 1 still differs.
- **Not the peephole.** Neutered, 1.19x, mark 1 still differs.
- **Not FRACLINE's MIR.** Diffed across all 53 stages: the tail block changes
  once, a CSE reusing one load of 0.0 for `re` and `im`. `y`'s cell is
  untouched end to end. The emitted entry, loop, latch and tail match BC's
  raise operation for operation, including which parameter each `bx` and `si`
  holds and `B$FCMP`'s operand order, and the x87 stack balances in both.

So it is in lowering or allocation, not in a pass -- which makes it mechanism
1 and a shared mechanism rather than a patch. `floatalloc` cannot be bisected
the way the others can: neutered it refuses at `fild`, because assigning the
x87 stack is what it is for.

### Where the instructions in a hot loop actually go

Innermost loops, every one in the demo, by what put the instruction there:

| | qbdemo, 1186 | deedlines, 7364 |
| --- | --- | --- |
| real work in registers | 57.8% | 65.9% |
| backend inserted | 24.1% | 20.7% |
| the program's own op with a local as operand | 10.5% | 5.4% |
| the program's own load/store of a local | 7.7% | 7.9% |

The backend adds about a fifth of every hot loop, and inserted
register-to-register moves (5.1%) outweigh spills (4.4%). That is the largest
addressable bucket and it is mechanism 2.

The rest is the shape of the problem. Between a half and two thirds of what
runs in a hot loop is BC's own instruction selection, reproduced unchanged,
and every win recorded in this document is subtractive -- a redundant reload,
a dead store, a duplicate home, a spilled counter. Deleting all of the
backend's own overhead would leave that untouched.

**Not the x87 stack either.** Absolute depth cannot be checked here -- nothing
records how many stack entries a runtime float call consumes, and a model that
guesses flags UNWHITEFADE and WHITEFADE, whose output is byte-identical. Run
the identical model over BC's raise and over the emitted code and the blind
spot cancels: the flag sets agree for every body in qbdemo, FRACLINE included.
So the stack discipline is reproduced faithfully and the divergence is at the
value level, in which location holds what.

Eliminated so far, each by a build and a run or by a differential: every MIR
optimization, the peephole, FRACLINE's raise, FRACLINE's lowering read against
BC's operation for operation, and the x87 stack discipline of every body.

## Where the failure actually is

## Where the failure actually is

Measured on qbdemo's 32 innermost loops, BC's raise against our emitted code,
matched by the BC bytes each op still covers (`scratchpad/wall.py`):

| | instructions | operands touching memory |
|---|---|---|
| BC | 1205 | 595 (49.4%) |
| ours | 1186 | 533 (43.8%) |

**1.02x.** Nineteen instructions across every hot loop in the program. The
1.26x the demo reports is won outside the loops, which is where the time is
not.

### What the phases do to it

Instruction count in those same loops after each LIR phase, which is rule 4
answering a question that was being reasoned about instead:

| phase | insns | reg-to-reg moves |
|---|---|---|
| lowered | **1012** | 3 |
| floatalloc | 1032 | 3 |
| phielim | 1111 | 75 |
| twoaddr | 1319 | 280 |
| coalesce | 1168 | 129 |
| regalloc | 1244 | 83 |
| peephole | 1186 | 63 |

Lowering hands allocation 1012 instructions against BC's 1205 -- 1.19x. The
conversion to six registers and two-address form costs 174 of it back, and
the win is gone. Both halves are small: the optimizer finds 16% redundancy,
allocation spends 17%.

### What was tried and did not move it

Each of these is a mechanism, measured, and each is worth about one percent.
Recorded so they stop being re-proposed.

| | effect on the 1186 |
|---|---|
| frame slots disjoint from every indexed operand | 1182 (-4) |
| optimistic coalescing (no Briggs refusal) | 1178 (-8), spills 48 -> 63 |
| strength reduction refusing more counters (`_RESERVE` 2->4) | 1193 (+7), spills 48 -> 57 |
| float cell reuse, the subset with no rounding question | -22 available |
| rematerializing constant-advanced counters | -8 available |
| a seventh allocatable register | 1158 (-28), spills 48 -> 30 |

Aliasing in particular should stop being the suspect: that relaxation is the
strongest the oracle could have and it is the fourth one to buy nothing.
Refusing strength reduction is worse than spilling its counters, so the
budget added for it is not the lever either -- the multiply chain it replaces
costs more than the reload, add and store that carry it.

### What the residue is made of

The 1186, by what the instructions are:

  * 993 of 1123 register operands are 16-bit. 71 are 32-bit.
  * **0 memory operands carry a scale factor.** 139 are indexed, all at
    scale 1.
  * 441 instructions (37%) are `mov`. 38 are `shl` by 1 to 3 and 87 are
    register `add`s -- 11% of the loop is address arithmetic that a 386
    `[base+index*scale+disp]` operand exists to absorb, against 16 `lea`.

So the residue is not redundancy that was missed. It is BC's instruction
selection, passed through: 8086 forms, 16-bit widths, addresses computed
longhand. Deleting redundancy is worth 16% and allocation spends 17%, which
is the whole of the 1.02x. The 3x to 17x in `docs/targets.md` is re-selection
-- and there is no stage that re-selects. Every pass either deletes an
operation or rewrites it in place, which is why seven fixes in one session
each landed between 0.1% and 1%.

## The target scoreboard cannot score a loop transformation

`unroll` is restricted to floating loops -- `if not any(op.floating for op in
latch.ops)` -- and `loopclone.peeled` is the general cloner sitting unused
beside it, already accepting a body with BRANCH and SWITCH in it. Routing
non-floating constant-trip loops through it builds every fixture and moves
the scoreboard a long way: segld 1.56x to 0.33x, spill 1.16x to 0.17x,
nested 1.17x to 0.66x, matrix 1.03x to 1.00x, hotlpx 2.06x to 1.82x, lngmxx
1.57x to 1.42x.

None of it is real. A hand-derived reference is the best code a person could
write, so beating one by 6x is not a result -- it is `_cost`'s own stand-in
being removed. The model weights a loop by ten per nesting level "as a
stand-in for a trip count nothing here knows"; unrolling a three-trip loop
replaces one body at weight ten with three at weight one, which is 0.3x for
free and measures nothing.

Emitted instruction counts say what actually happened:

| | before | after |
|---|---|---|
| segld | 45 | 940 |
| spill | 41 | 882 |
| matrix | 54 | 428 |
| nested | 49 | 396 |
| stride | 24 | 230 |
| jumps | 115 | 115 |

Twenty times the code, and nothing at all for `jumps`, which is the target
that motivated it. Reverted.

So the instrument has to be fixed before this optimization can be worked on:
`_cost` must weight a loop by `unroll._trips`'s answer where there is one,
and fall back to ten only where there is not. Until then no loop
transformation can be scored, which is the likeliest reason the five targets
above 1.5x have resisted -- four of them (HOTLPX, PRESSX, FPCSEX, LNGMXX)
already carry a hand-written trip override in `_TRIPS`, and everything else
is being scored against a guess.

### Why `jumps` does not unroll

Followed to the bottom, because 2.95x is the worst documented target.
`induction.basics` reports no counter, so the trip count is unknown and both
expansions refuse. The header phi is there -- `v1_3 := phi 0x30:v1_5,
0x127:v1_29` -- and the loop arm is `inc v1_17`, where `v1_17` is still a
`LOAD` of `[seg:5+0x12]`, the counter's own cell. Nothing clears
availability in that loop and the cell is in `promotable()`, so promotion
created the variable and the phi and then left this one read in memory.
`basics` requires the step to apply to the phi's own result and chases
copies on the result side only, so a read that came back through memory
breaks the recurrence.

That is the chain: promote leaves one read, induction sees no recurrence,
and every consumer of a trip count -- unroll, strength reduction, the cost
model's own weighting -- gets nothing. Fixing it is one mechanism at the
first link, not five at the ends.

## The `ON GOTO` dispatcher was unbounding every cell in its program

`jumps` was the worst documented target at 2.95x. Followed to the bottom,
through four instruments each of which had to be corrected first:

`induction.basics` reports no counter, so no trip count, so neither
expansion runs. The header phi is there and its loop arm is `inc v1_17`,
where `v1_17` is a `LOAD` of `[seg:5+0x12]` -- the counter's own cell. So
the counter never left memory. `promote` reports no candidate at all for the
body, and with the pipeline's own dgroup and bounds rather than an empty set
(the first probe passed `frozenset()` and answered a different question):
touches and widths pass, and the availability filter is what drops it.

What clears availability is one call:

    [avail] 0x3100000001 0x00052 CALL removes ['[seg:5+0x12]', '[seg:5+0x8]']
            via stores=[('None', 0, None)]

`beyond=None` -- no bound at all, so the store aliases the whole program.
The call is `B$OGTA`, the `ON GOTO` dispatcher, and it sits inside the loop.
Every `B$P*` output routine beside it in that same loop carries
`beyond=(5, {...})` and is bounded.

The difference is one condition in `raising_call_memory.reachable`: it
admits `Control.RETURNS` and `Control.NEVER`, and `B$OGTA` is
`Control.INLINE_TABLE`. Its `writes` level is 3, the same as every routine
that is bounded. Where a routine leaves control is a different question from
what it can write, and `beyond` answers only the second -- an INLINE_TABLE
call resumes at one of the table's targets, which the raise already models
as the dispatch block's own successors. So the bound holds across it exactly
as it holds across a return.

**Before and after**, emitted instructions for jumps-p-g2, 115 to 60:

| removed | |
|---|---|
| 12 | `push r40.4` |
| 12 | `pop r24.2` |
| 6 | `pop r22.2`, 6 `pop r21.2` |
| 6 | `mov [seg:5+0xe],r21` and 6 `mov [seg:5+0x10],r24` |
| 6 | reloads of `[seg:5+0xa]` and `[seg:5+0x6]` |

and the `and`/`or`/`xor`/`sub`/`sbb` rows now read registers where they read
memory. `a`, `b` and `r` stay in registers across the loop, which is what
promotion is for and what one unbounded call was preventing.

Cost 2192 to 1106, **2.95x to 1.49x**, and the emitted count moved the same
way -- 1.92x against the cost model's 1.98x. Both directions agreeing is the
check the unroll attempt above failed.

Across the support matrix, 31 fewer instructions on every non-event variant
of all three families: jumps-p-g2, -p-noO, -p-ot, -q-O, -q-noO, -v-g2, -v-g3,
-v-noO, -v-plain. Event-enabled builds are unchanged, which is expected --
an event check makes the call a barrier. `jumptable` emits one more pseudo-op
and the same 8 real instructions in the same 761 bytes. Every other
documented target is unchanged. No demo uses `B$OGTA`, so none can be
affected.

Four documented targets remain above 1.5x: hotlpx 2.06x, harr 1.77x,
lngmxx 1.57x, segld 1.56x.

## `merges` was standing in for "not analysable", and it is every accumulator

`loopexit.py` is the mechanism two documented targets needed and it was
already written: "Evaluate affine exit values and delete finite,
side-effect-free counted loops ... a fixed increment sums to N * step; an
affine increment also contributes N(N-1)/2 times its stride." It runs from
`Strength.transform`, so it was reached on every body, and it never fired.

`_exit_terms` could not linearise the accumulator:

    [terms] phi v10_1 update=v10_3 linear=None

`_linear` refuses an operation with `op.merges` set. That is the two-address
tie -- which use shares a register with which definition -- and it says
nothing about whether the operation is a linear function of its own
arguments. `op.args` and `op.results` describe the arithmetic either way, and
`_linear` already admits only COPY, ADD, SUB, INCREMENT and DECREMENT with a
single result it asked for, so nothing wider can reach it. BC writes every
accumulator as a two-address `add`, so the check refused every accumulator
in the corpus. `_disposable` carried the same check, where deleting the block
removes the tied use and the tied definition together.

Removing it from both leaves hotlpx's loop **gone**:

    imul r24 <- r24,[seg:5+0x6]    ; n*k
    lea  r21 <- [r40+r40*4]        ; x5
    shl  r21 <- r21,2              ; x20
    add  r21 <- r21,210
    mov  [seg:5+0xc] <- 21         ; i
    mov  [seg:5+0xa] <- r21        ; s

which is `docs/targets.md`'s own reference term for term -- `s = (20 *
((n*k) mod 65536) + 210) mod 65536` and `i = 21`. `loops()` over the emitted
code counts zero loops in hotlpx and in lngmxx.

**hotlpx 2.06x to 1.13x, lngmxx 1.57x to 1.16x**, and pressx 1.37x to 1.20x,
rotate 1.38x to 0.89x, spill 1.16x to 0.43x, hotlop 0.79x to 0.37x, lngmix
0.91x to 0.58x, press 0.66x to 0.44x. No ratio anywhere is worse.

Static instruction counts go the other way -- hotlpx 31 to 38, lngmxx 45 to
49 -- and that is what loop deletion looks like: a body that ran twenty
times is replaced by straight-line arithmetic that runs once. The unroll
attempt recorded above failed exactly this check, because there the body
still ran the same number of times and only the cost model's weighting had
changed. The distinguishing evidence is the loop count in the emitted code,
not either number on its own.

Two documented targets remain above 1.5x: harr 1.77x and segld 1.56x, both
with six refusals.

## Two counters that were always equal

harr and segld are the same fixture twice: a dynamic array subscripted
inside a nested loop. Their own comments name the cause as the descriptor
and segment being reloaded every pass, and by now neither is -- the segment
is hoisted and the offsets are strength-reduced. What was left in segld's
inner loop is eight instructions:

    mov [es:bx+0x0] <- r22     ; a(i) = i
    mov r23 <- [es:bx+0x0]     ; the cell just written, read back
    add r24 <- r24,r23
    inc r22
    add r27 <- r27,2           ; two offsets into `a`
    add r28 <- r28,2           ; always equal to the first
    cmp r22,20
    jle

`mir.same_bytes` refuses `Space.FAR` unless both references carry the same
proven `allocation`, and both of these do -- `arrayfacts` annotates them by
`mir-r01-cse`. What it then refuses on is `one.base != other.base`: the
store's address and the load's address are different SSA values, because BC
computes the subscript once per use. So the reload of the cell just written
could not be forwarded, and that is `opportunity.py`'s "load of a cell just
written" -- three of them in harr.

The two offsets have the same start and the same step, so they are the same
value at every iteration. `ivshare` did not merge them: it looks for a start
defined by `add source,const`, and two identical counters share a start
rather than computing one from the other. Nothing else merges them either,
because a phi is not one of the computations `transform`'s value numbering
considers.

Handling the degenerate case -- same start, same step, offset zero, so a
COPY of the canonical phi rather than an ADD -- removes the duplicate `add`
and, because the two references then share a base value, lets the reload
forward as well.

**harr 1.77x to 1.12x with its six refusals gone, segld 1.56x to 1.05x with
its six gone.**

## Every documented target is now within 1.5x

| | was | now |
|---|---|---|
| jumps | 2.95x | 1.49x |
| hotlpx | 2.06x | 1.13x |
| harr | 1.77x | 1.12x |
| lngmxx | 1.57x | 1.16x |
| segld | 1.56x | 1.05x |

The highest remaining ratio across the whole scored suite is jumps at 1.49x.
Nothing scored got worse: pressx 1.37x to 1.20x and rotate 1.38x to 0.89x
improved alongside, and every other program is unchanged. divmod, fpemu and
procs have no target; fpcsex stays PROVISIONAL for the reason already
recorded against it.

Three changes, each one condition:

  * `raising_call_memory.reachable` admits `Control.INLINE_TABLE`. Where a
    routine leaves control is not what it can write.
  * `loopexit._linear` and `_disposable` no longer refuse on `op.merges`.
    That is the two-address tie, and BC writes every accumulator as one.
  * `ivshare` merges two counters with the same start and step.

None of them is a program-specific patch and none names a machine.

### The gate

487 objects in `fixtures/omf`, all three compiler families and every
variant: **0 refused, 132 changed, all 132 smaller, none larger.** The
changes run across families as the mechanism should -- harr-p, -q and -v,
hotlop, hotlpx and the rest at the same sizes in each.

Object size is not the claim (the goal says so outright); the ratios are.
Size is the no-regression evidence beside them.

All four demo objects -- deedlines, oimad, qbdemo, qbfrac -- emit an
identical instruction count before and after all three changes, so no demo
behaviour or timing can move and no run is called for.

Two things to keep in view rather than file as finished. `jumps` at 1.49x is
inside the gate by one hundredth, so it is the first thing any later change
should be re-measured against. And the loop-deleted programs emit *more*
static instructions than before -- hotlpx 31 to 38, lngmxx 45 to 49 -- which
is what replacing a body that ran twenty times with arithmetic that runs
once looks like; the check that separates it from the reverted unroll is the
loop count in the emitted code, which is zero for both.

## Mechanism 2, and what its headroom actually is

The phase table is unchanged by any of the three fixes above, because none
of them touches a demo: lowering hands allocation 1012 instructions for
qbdemo's innermost loops and emission delivers 1186. Where the 174 goes:
floatalloc +20, phi elimination and the two-address fixup +136 net of
coalescing, allocation +76, peephole -58.

Five levers tried against it, each measured:

| | effect on the 1186 |
|---|---|
| optimistic coalescing (no Briggs refusal) | -8, spills 48 -> 63 |
| coalescing innermost-loop first | 0 (coalesce 1168 -> 1170, allocation absorbs it) |
| a seventh allocatable register | -28, spills 48 -> 30 |
| a fifth | +50, spills 48 -> 85 |
| eliminating every immediately-dead definition | 4 exist, all `pop` pairs |

211 of the 268 copies coalescing leaves behind have ends that do **not**
interfere -- Briggs refused them rather than interference forbidding them.
Both attempts to spend that differently came to nothing, and the register
sensitivity says why: the binding constraint is six registers, not the order
or the boldness of the joins. Joining more classes makes them uncolourable
and the allocator spills instead, which is the trade the first row measures.

An earlier note in this file blamed constant rematerialization for 4.1% of
each hot loop, reading `mov r23,968; mov r23,969; mov r23,969` off an
example dump. That was an artefact of printing examples keyed on `covers`,
which is the same address for everything inserted at one point. Asked
properly -- an instruction whose whole register result the next one
overwrites without reading -- qbdemo has four, and all four are `pop` pairs
whose stack-pointer effect the question does not model. There is no
redundant constant materialization to remove.

So the remaining 174 is the cost of SSA-to-six-registers with two-address
operations in this pipeline's shape, and reducing it needs a restructuring
rather than a tuning: allocation over SSA with phi congruence resolved after
colouring, or George-Appel's iterated coalescing with an undo. The
allocator's retry loop already splits before spilling, so the undo has
somewhere to live -- what is missing is that `coalesce` renames its joined
values and keeps no record of which copy created which class, so nothing
downstream can separate one again.

## The all-compiler scan, and the one thing it found

`target-coverage-current.md` says of its own numbers: "This is not an
all-compiler scan or runtime correctness gate." Everything above was scored
on ordinary PDS `/G2`, so the scan was run: `--targets` over all 487
fixtures, 329 scored rows across QuickBASIC, PDS and VBDOS and every
variant.

Four rows above 1.5x, all of them `jumps`: **q-O, q-O-zd and q-noO at 1.57x,
v-plain at 1.55x**. The PDS-only view had hidden them. Contracts are
identical across the three families, so the difference is not the ABI --
it is what BC chose to emit. QB's BC writes

    je   dc
    jmp  f5        ; a block of its own
  dc: ...

where PDS's BC writes the inverted branch and no jump. `layout` keeps BC's
block order and only ever *adds* jumps, so the choice was preserved.

`_threaded` reverses such a branch, sends it where the jump went, and leaves
the branch's own target as the fall-through. The emptied block keeps its
address and its `covers` -- BC's bytes must stay owned -- and emits nothing.

One thing had to change with it: `_fallthroughs` compared a block's
fall-through against the *immediately* next block, so it put a `jmp` straight
back over every block just emptied, and threading appeared to do nothing at
all. `_following` now skips blocks that emit no bytes, which is what falling
through one means.

487 objects, 0 refused, **19 changed, all smaller, none larger**. jumps-q-O
and v-plain 1.57x/1.55x to **1.54x**, p-g2 1.49x to 1.48x.

### What those four still need

Still 1.54x against a 1.5x gate. The rest of the QB/PDS difference is three
instructions:

    mov r21,0
    mov [bp-0x2],r21     ; a frame temporary holding a constant
    ...
    mov r21,[bp-0x2]
    push r21             ; where PDS emits `push 0`

`promote` should take that cell -- a fixed FRAME address, touched twice, one
width -- and every PRINT call between the store and the load clobbers it,
because `_out_of_reach` bounds a call's writes only over `Space.SEGMENT`.
The same measured fact covers frame locals: a runtime routine writes its own
data and whatever the program handed it a pointer to. A frame slot whose
address is never taken is not that.

The analysis for it is written and connected to nothing:
`qbopt/analysis/frameescape.py` computes `exposed`, the frame offsets whose
address escapes. The place to put its answer already exists too --
`MemRef.excludes`, "exact byte ranges this effect cannot reach", which
`arrayfacts` already fills for statics and `_excluded` already reads.

Not done here, deliberately. A frame address escapes through VARPTR, VARSEG,
by-reference argument passing, and -- as `frameescape`'s own first comment
warns -- runtime frame walking and callbacks, none of which its `origins`
claim to cover. Wiring an escape analysis into an aliasing bound on the
strength of a 1.54x-to-1.5x gap is how a silent miscompile gets shipped. It
needs the obligations discharged first, and that is the next piece of work.

### Measured: the frame cannot be bounded the way DGROUP is

The `beyond` bound exists because a measurement justified it --
`tools/runtime_writes.py`, reading a linked image, finds that no runtime
write names a cell in BC_DATA, so a program's variable is reachable only
through a pointer the program handed over. The proposal above was to extend
the same bound to frame slots. Run the same instrument and it refutes it.

Four linked images, PDS and VBDOS:

| image | fixed writes in STACK | writes through `ss` |
|---|---:|---:|
| B_BOOLS | 9 | 10 |
| B_MATRIX | 3 | 10 |
| B_LNGMIX | 0 | 17 |
| B_DIVMOD | 0 | 10 |

Every image agrees on BC_DATA -- "No runtime write names a cell in
BC_DATA" -- and every image contradicts the frame: the runtime writes the
stack segment, at fixed addresses in two of the four and through
`ss`-relative addresses in all four. A frame slot is `ss`-relative, and
relating a runtime `[ss:...]` write to a caller's `[bp-2]` needs sp's
relation to bp inside the callee, which nothing at this layer can see.

`module.may_alias` already says exactly this and gives the same reason: "A
stack slot against a frame slot is a different question and stays
conservative -- both are in the same region and their displacements are
against different registers." Relaxing the call bound over frame cells
would have contradicted a rule already in the file for a stated reason.

So the four `jumps` rows stay at 1.54x, and the reason is a measured
property of the runtime rather than a judgement about risk. Closing that
0.04x needs the three instructions removed some other way -- the constant
never routed through a frame temporary in the first place, which is a
question about how QB's BC passes a literal argument and therefore about the
raise, not about aliasing.

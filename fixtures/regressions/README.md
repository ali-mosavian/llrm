# Focused regression objects

`udtacc-{q-O,p-g2,v-g3}.obj` are real compiler output from `udtacc.bas`.
Two LONG fields of an input-selected record accumulate 3 and -5 seven times;
all three baseline/optimized pairs print 21, -35 and DONE. PDS exposed an
address-decoding gap: `[si]` was unnamed while `[si+2]` was recognized, leaving
the second field split. An absent displacement is now literal zero with its
base retained; no relocation is invented. The existing segment/index refusals
are unchanged.

PDS before/after (STEPY names its relocation):

```asm
; before                     ; after
mov ax,[si]                  mov eax,[si]
mov cx,[si+2]                mov ecx,[STEPY]
add ax,[STEPY]               add eax,ecx
adc cx,[STEPY+2]
mov [bp-14h],bx              mov [bp-14h],bx
mov [si],ax                  mov [si],eax
mov [si+2],cx
```

PDS modeled cost falls **1258 → 1092**, object **1299 → 1288 bytes**.
Five focused regressions failed before the fix; 182 lift/whole-value tests
pass afterwards. This establishes whole-field recognition, not SROA or
cross-iteration promotion: the loop still loads and stores both fields.
There is no registered optimal target for this focused fixture.

`chain-stack-q-O.obj` is real QuickBASIC 4.5 `/O` output from
`suite/chain.bas`, captured during the stack-argument recovery work. Its
constant-divisor remainder feeds another remainder and is reused later.
Unlike the older chain fixture, this compilation has relocated memory
pushes in the recovered argument sequence. Losing their relocation makes
`CONST2` print 0 instead of 13106.

`nbody-stack-p-g2.obj` is real PDS 7.1 `/G2` output from `suite/nbody.bas`.
Its inner-loop multiply has an outer division's constant argument already
pushed below its own arguments. Recovery must keep that argument separate.

`ldpre-{q-O,p-g2,v-g3}.obj` are real QB 4.5 `/O`, PDS 7.1 `/G2` and
VBDOS `/G3` output from `ldpre.bas`, compiled through `tools/e2e.py`.
Expected answers are 35, 70 and -28. Load PRE keeps the true arm's freshly
stored `x` in a value and loads `x` only on the false arm. Before:

```asm
true:  mov [x],eax
       jmp join
false: ; update y
join:  mov ebx,[x]
       mov eax,ebx
       shl eax,3
       sub eax,ebx
```

After (QB; symbolic addresses substituted for relocation fields):

```asm
true:  mov [x],eax
       jmp join
false: ; update y
       mov eax,[x]
join:  mov ebx,eax
       shl ebx,3
       sub ebx,eax
```

The true path saves one read; the false path adds none. QB object size changes
1136 → 1138 bytes due to encoding/relocation differences, not added work on the
true path. This is a focused capability regression, not a new benchmark target.

`ldcrit-{q-O,p-g2,v-g3}.obj` are real compiler output from `ldcrit.bas`,
compiled through `tools/e2e.py` with the same flags. Answers: 35, 70, -28.
Unlike LDPRE, there is no ELSE body. PDS/VBDOS branch directly to the common
load; PRE must split that conditional edge. PDS before/after, with relocation
fields named and unchanged code omitted:

```asm
; before                     ; after
test ax,ax                   test ax,ax
je join                      je missing
mov eax,[y]                  mov eax,[y]
add eax,1                    add eax,1
mov [x],eax                  mov [x],eax
join:                        join:
mov ebx,[x]                  mov ebx,eax
mov eax,ebx                  shl ebx,3
shl eax,3                    sub ebx,eax
sub eax,ebx                  ; remaining body, ending in B$CENP
                             missing:
                             mov eax,[x]
                             jmp join
```

The true arm saves one read; the false arm adds one jump. PDS object size
grows 1102 → 1106 bytes. QB's emitted assembly is unchanged: its original
control-flow shape already lets dedicated-edge PRE remove the repeated read.
This proves critical-edge capability, not universal profitability. Three
runtime cases pass on each compiler. The first PDS attempt was refused because
lowering failed to select the inserted operand-free MIR jump; the emitted-path
regression also requires successful LIR emission, never unchanged fallback.

`localp-{q-O,p-g2,v-g3}.obj` are real compiler output from `localp.bas`,
compiled through `tools/e2e.py`. The procedure-local loop prints 28, then main
prints DONE. Before frame-promotion work began, PDS's optimized program produced
no output before the timeout while BC printed both lines. CFG merging removed
main's jump across the physically interleaved SUB; termination remained in the
object but was unreachable. PDS before/after (only the affected control flow):

```asm
; broken                     ; repaired
call B$PESD                  call B$PESD
nop                          nop
; falls into ACCUMULATE       jmp termination
ACCUMULATE:                  ACCUMULATE:
; procedure body             ; procedure body
retf 2                       retf 2
termination:                 termination:
call B$CENP                  call B$CENP
```

An unowned gap is not an empty path: merging one body's blocks cannot erase a
jump over another body while layout still retains its physical position.
The original failure and repaired output were checked with per-stage dumps;
all three compilers' linked executables now print 28 and DONE. Frame-field
promotion is still unimplemented; this fixture uncovered a correctness defect
before any change to promotion.

The subsequent promotion change admits fixed frame fields to the same
write-through analysis as module globals. It establishes the accumulator words
from the full-width zero initializer; existing store sinking then moves the
two writes to the exit. PDS's loop arithmetic changes as follows (counter
extension, increment and termination comparison omitted):

```asm
; before                         ; after
mov dx,bx                        add dx,bx
add dx,[bp-16h]                  adc di,cx
adc cx,[bp-14h]                  mov cx,di
mov [bp-16h],dx                  ; stores now at the loop exit
mov [bp-14h],cx
```

QB/PDS modeled whole-program cost: **772 → 584**; VBDOS: **764 → 580**.
Object sizes: QB 1089 → 1090, PDS 1116 → 1117, VBDOS 1253 → 1257.
All three linked programs still print 28 and DONE. Unknown calls and
overlapping frame writes invalidate reuse; disjoint field writes do not.
This is scalar frame-field promotion, not a completed SROA implementation.

Exposing sign extension before LONG-pair recognition removes the remaining
half-word arithmetic. PDS's next before/after, omitting loop control:

```asm
; before                      ; after
movsx edi,bx                  movsx edx,cx
push edi                      add edx,eax
pop di                        mov eax,edx
pop di
add dx,bx
adc di,cx
mov cx,di
```

MIR now receives one LONG addition and one LONG store. The existing allocator
and frame promotion can keep the whole accumulator instead of its halves.
Modeled costs fall again: QB/PDS **584 → 380**, VBDOS **580 → 368**.
Object sizes: QB 1090 → 1087, PDS 1117 → 1114, VBDOS 1257 → 1249.
All three linked outputs remain 28 and DONE. The raise-time regression was
observed failing on all three objects before moving sign-fill recognition.

Two-address selection now uses result-copy affinity to break ties between dead
commutative operands. Choosing the accumulator lets ordinary coalescing remove
the backedge copy; live operands, constrained instructions and noncommutative
operations retain their existing rules. PDS before/after:

```asm
; before                      ; after
movsx edx,cx                  movsx edx,cx
add edx,eax                   add eax,edx
inc cx                        inc cx
mov eax,edx                   cmp cx,bx
cmp cx,bx                     jle loop
jle loop
```

QB/PDS modeled cost **380 → 360**, VBDOS **368 → 348**; all objects shrink
three bytes. All three linked outputs remain 28 and DONE. This is a backend
operand-selection change: MIR's arithmetic and value identities are unchanged.

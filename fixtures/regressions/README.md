# Focused regression objects

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

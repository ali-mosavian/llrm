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

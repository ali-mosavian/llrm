# Reuse a recurrence for loop termination

`src/optimize/indvars.rs` removes a controlling induction variable when it is
otherwise used only by its own update and constant exit uses. An existing
recurrence already needed by the body supplies the termination test.

The original signed comparison must prove a positive finite trip count N
and a non-wrapping final update. For an alternative w-bit recurrence with
stride S, N must be smaller than its modular period 2^w/gcd(S,2^w).
Consequently the recurrence cannot equal its final value on an earlier
iteration, even if the alternative itself wraps. The new test is equality
or inequality against start + N*S. No target architecture enters this proof.

Only canonical two-block loops with a unique latch, preheader and exit
predecessor qualify. Body observations of the old counter, other uses of
comparison flags, and unsupported phi/live-out uses prevent the rewrite.
The old counter's final constant is materialized at the exit.

## HARR

Current HARR already hoists its segment load and advances both array
addresses with additions. This change removes the remaining separate
column-counter update. Symbolic names below describe the actual VBDOS
register allocation (`sum=CX`, `element=SI`, `pointer=DI`):

```asm
; Before inner loop              ; After inner loop
mov [es:di],si                   mov [es:di],si
add cx,si                        add cx,si
inc dx
add si,1                         add si,1
add di,2                         add di,2
cmp dx,10                        cmp si,dx
jle inner                        jne inner
```

DX now holds an invariant bound computed once per row from the initial SI
plus ten. The observable final column value is still stored as eleven.
This removes one instruction per inner iteration, but adds setup and exit
work. It does not free a physical register: the bound occupies the old
counter's register. No fresh timing claim is made.

## Evidence

HARR, MATRIX and NESTED change in the PDS fixture emission comparison; no
other PDS /G2 suite object changes. All three programs pass actual QB /O,
PDS /G2 and VBDOS /G3 runs. HARR prints 1100. 58 focused counter, loop-exit
and architecture checks pass.

The initial implementation exposed an emission-order mistake: a bound
inserted at the beginning address of its preheader was emitted before its
input was initialized, producing HARR answers 18183 (QB/PDS) and 12327
(VBDOS). The insertion now uses the final preheader operation's address.
An emitted-code regression fails with the bad placement restored. The
counter-elimination fixture regressions also fail with the pass disabled.

Before dumps: `/tmp/qbopt-harr-current/s81-asm-emitted.txt` and corresponding
MIR files. After dumps and passing HARR runs:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-indvars-fixed-no1fz7b5`.
Passing MATRIX/NESTED runs:
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-indvars-affected-ikexjl97`.

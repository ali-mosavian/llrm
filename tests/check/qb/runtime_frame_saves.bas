' RUN: llrm-qb %s --dialect qb45 --runtime qb45 --runtime-frames -O2 -S -o /dev/stdout
' B$ENRA saves SI and DI and B$EXSA restores them. Shrink-wrapping moved the procedure's own
' saves past its first runtime call, and the native shell no longer stripped:
' "LOADQBINSIDE: native frame prefix changed shape" (deedlines' TSC.BAS).
' CHECK-LABEL: LOADQBINSIDE proc
' CHECK: call far ptr B$ENRA
' CHECK-NOT: push si
' CHECK-NOT: push di
' CHECK: LOADQBINSIDE endp
DECLARE SUB loadqbinside ()
DIM SHARED sp%(10000)
loadqbinside
SUB loadqbinside
a$ = "qbrules.spr"
c$ = " "
OPEN a$ FOR BINARY AS #1
OUT &H3C8, 128
FOR i% = 0 TO 383
GET #1, , c$: OUT &H3C9, ASC(c$)
NEXT i%
xg% = 100
yg% = 100
i% = 0
FOR y% = 1 TO yg%
FOR x% = 1 TO xg%
GET #1, , c$: sp%(i%) = ASC(c$)
i% = i% + 1
NEXT x%
NEXT y%
CLOSE #1
END SUB

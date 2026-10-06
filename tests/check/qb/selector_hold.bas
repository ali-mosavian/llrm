' RUN: llrm-qb %s --dialect qb45 --runtime qb45 -Os --cpu 486 -fno-inline-functions -S -o /dev/stdout
' A loop that fits the selector registers holds its arrays' segments in them: loaded once before it, not before
' each use. With typed writes the segment loads proved stable, and a tie in static loads made them per trip
' (+300 instructions at -Os, same bytes).
' CHECK-LABEL: BENCHPARTICLE proc
' CHECK: call far ptr ADVANCE
' CHECK: mov es, word ptr PX%+2
' CHECK: mov fs, word ptr PY%+2
' CHECK: mov gs, word ptr PZ%+2
' CHECK: L1_{{[0-9]+}}:
' CHECK-NOT: mov es, word ptr PX%+2
' CHECK: BENCHPARTICLE endp
DEFINT A-Z
DECLARE SUB Advance (n)
DECLARE FUNCTION BenchParticle& ()
'$DYNAMIC
DIM SHARED px(-50 TO 50), py(-50 TO 50), pz(-50 TO 50)
DIM SHARED vx(-50 TO 50), vy(-50 TO 50), vz(-50 TO 50)
PRINT LTRIM$(STR$(BenchParticle&))
END

FUNCTION BenchParticle&
    FOR i = -50 TO 50
        px(i) = i: py(i) = 2 * i: pz(i) = -i
        vx(i) = i AND 7: vy(i) = 3 - (i AND 3): vz(i) = 1
    NEXT
    FOR t = 1 TO 100: Advance 50: NEXT
    s& = 0
    FOR i = -50 TO 50
        s& = s& + px(i) + 3& * py(i) + 7& * pz(i)
    NEXT
    BenchParticle& = s&
END FUNCTION

SUB Advance (n)
    FOR i = -n TO n
        px(i) = px(i) + vx(i)
        py(i) = py(i) + vy(i)
        pz(i) = pz(i) + vz(i)
    NEXT
END SUB

DEFINT A-Z
DECLARE SUB Advance (n)
'$DYNAMIC
DIM SHARED px(-50 TO 50), py(-50 TO 50), pz(-50 TO 50)
DIM SHARED vx(-50 TO 50), vy(-50 TO 50), vz(-50 TO 50)
FOR i = -50 TO 50
    px(i) = i: py(i) = 2 * i: pz(i) = -i
    vx(i) = i AND 7: vy(i) = 3 - (i AND 3): vz(i) = 1
NEXT
FOR t = 1 TO 100: Advance 50: NEXT
s& = 0
FOR i = -50 TO 50
    s& = s& + px(i) + 3& * py(i) + 7& * pz(i)
NEXT
OPEN "PARTICLE.OUT" FOR OUTPUT AS #1
PRINT #1, "RESULT="; s&
CLOSE #1
END

SUB Advance (n)
    FOR i = -n TO n
        px(i) = px(i) + vx(i)
        py(i) = py(i) + vy(i)
        pz(i) = pz(i) + vz(i)
    NEXT
END SUB

' known: #358
' Six-body SINGLE n-body; checksum = weighted sum of truncated (pos*64).
DEFINT A-Z
DECLARE FUNCTION BenchFpbench& (steps AS INTEGER)

PRINT LTRIM$(STR$(BenchFpbench&(20)))
END

FUNCTION BenchFpbench& (steps AS INTEGER)
    DIM px(5) AS SINGLE, py(5) AS SINGLE, vx(5) AS SINGLE, vy(5) AS SINGLE
    DIM dx AS SINGLE, dy AS SINGLE, d2 AS SINGLE, f AS SINGLE
    DIM ax AS SINGLE, ay AS SINGLE
    DIM acc AS LONG, stp AS INTEGER, b AS INTEGER, o AS INTEGER

    FOR b = 0 TO 5
        px(b) = b * 7 - 15
        py(b) = b * 5 - 12
        vx(b) = 0
        vy(b) = 0
    NEXT b

    FOR stp = 1 TO steps
        FOR b = 0 TO 5
            ax = 0: ay = 0
            FOR o = 0 TO 5
                IF o <> b THEN
                    dx = px(o) - px(b)
                    dy = py(o) - py(b)
                    d2 = dx * dx + dy * dy + 1
                    f = 1 / d2
                    ax = ax + dx * f
                    ay = ay + dy * f
                END IF
            NEXT o
            vx(b) = vx(b) + ax
            vy(b) = vy(b) + ay
            vx(b) = vx(b) - vx(b) / 16
            vy(b) = vy(b) - vy(b) / 16
        NEXT b
        FOR b = 0 TO 5
            px(b) = px(b) + vx(b)
            py(b) = py(b) + vy(b)
        NEXT b
    NEXT stp

    acc = 0
    FOR b = 0 TO 5
        acc = acc + CLNG(FIX(px(b) * 64)) * (b + 1) + CLNG(FIX(py(b) * 64)) * (b + 7)
    NEXT b
    BenchFpbench& = acc
END FUNCTION

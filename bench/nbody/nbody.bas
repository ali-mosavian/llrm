' Four-body f64 kernel; DOUBLE arrays, INTEGER counters (BASIC has no unsigned).
DECLARE FUNCTION BenchNbody& (steps AS INTEGER)

PRINT LTRIM$(STR$(BenchNbody&(200)))

FUNCTION BenchNbody& (steps AS INTEGER)
    DIM x(3) AS DOUBLE, y(3) AS DOUBLE, vx(3) AS DOUBLE, vy(3) AS DOUBLE
    DIM dx AS DOUBLE, dy AS DOUBLE, scale AS DOUBLE
    DIM stp AS INTEGER, i AS INTEGER, j AS INTEGER

    x(0) = -1#: x(1) = 1#: x(2) = 0#: x(3) = 0#
    y(0) = 0#: y(1) = 0#: y(2) = -1#: y(3) = 1#
    vx(0) = 0#: vx(1) = 0#: vx(2) = .0125#: vx(3) = -.0125#
    vy(0) = -.0125#: vy(1) = .0125#: vy(2) = 0#: vy(3) = 0#

    FOR stp = 0 TO steps - 1
        FOR i = 0 TO 3
            FOR j = i + 1 TO 3
                dx = x(j) - x(i): dy = y(j) - y(i)
                scale = .00001# / (dx * dx + dy * dy + .125#)
                vx(i) = vx(i) + dx * scale: vy(i) = vy(i) + dy * scale
                vx(j) = vx(j) - dx * scale: vy(j) = vy(j) - dy * scale
            NEXT j
        NEXT i
        FOR i = 0 TO 3
            x(i) = x(i) + vx(i)
            y(i) = y(i) + vy(i)
        NEXT i
    NEXT stp
    BenchNbody& = CLNG(FIX((x(0) + y(1) + x(2) + y(3)) * 1000000#))
END FUNCTION

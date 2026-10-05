' Integer Q8 Mandelbrot work count: deterministic, with no floating ambiguity.
' No shift in BASIC: x*y >> 7 is floor division, written as \ with a bias for negatives.
DEFINT A-Z
DECLARE FUNCTION BenchMandel& ()

PRINT LTRIM$(STR$(BenchMandel&))

FUNCTION BenchMandel&
    DIM x AS LONG, y AS LONG, cx AS LONG, cy AS LONG, xx AS LONG, yy AS LONG
    DIM work AS LONG, t AS LONG
    work = 0
    FOR py = -12 TO 11
        FOR px = -16 TO 15
            x = 0: y = 0
            cx = CLNG(px) * 24 - 128
            cy = CLNG(py) * 24
            FOR iteration = 0 TO 31
                xx = (x * x) \ 256
                yy = (y * y) \ 256
                IF xx + yy > 1024 THEN EXIT FOR
                t = x * y
                IF t < 0 THEN t = t - 127
                y = (t \ 128) + cy
                x = xx - yy + cx
            NEXT iteration
            work = work + iteration
        NEXT px
    NEXT py
    BenchMandel& = work
END FUNCTION

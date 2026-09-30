FUNCTION Dot& (a() AS INTEGER, b() AS INTEGER, n AS INTEGER)
    DIM total AS LONG, i AS INTEGER
    FOR i = 0 TO n - 1
        total = total + CLNG(a(i)) * b(i)
    NEXT
    Dot& = total
END FUNCTION

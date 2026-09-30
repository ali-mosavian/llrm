FUNCTION Dot& (a() AS INTEGER, b() AS INTEGER)
    DIM total AS LONG, i AS INTEGER
    FOR i = 0 TO UBOUND(a)
        total = total + CLNG(a(i)) * b(i)
    NEXT
    Dot& = total
END FUNCTION

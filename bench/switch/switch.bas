' Same algorithm as switch.c.
DEFINT A-Z
DECLARE FUNCTION Weight& (k AS LONG)
DECLARE FUNCTION Route& (op AS LONG)
DECLARE FUNCTION BenchSwitch& (n AS INTEGER)

PRINT LTRIM$(STR$(BenchSwitch&(2000)))

FUNCTION Weight& (k AS LONG)
    SELECT CASE k
        CASE 0: Weight& = 3
        CASE 1: Weight& = 14
        CASE 2: Weight& = 15
        CASE 3: Weight& = 92
        CASE 4: Weight& = 65
        CASE 5: Weight& = 35
        CASE 6: Weight& = 89
        CASE 7: Weight& = 79
        CASE 8: Weight& = 32
        CASE 9: Weight& = 38
        CASE 10: Weight& = 46
        CASE 11: Weight& = 26
        CASE ELSE: Weight& = 1
    END SELECT
END FUNCTION

FUNCTION Route& (op AS LONG)
    SELECT CASE op
        CASE 0: Route& = 11
        CASE 37: Route& = 7
        CASE 148: Route& = 23
        CASE 333: Route& = 5
        CASE 592: Route& = 19
        CASE 925: Route& = 2
        CASE 1332: Route& = 13
        CASE ELSE: Route& = 1
    END SELECT
END FUNCTION

FUNCTION BenchSwitch& (n AS INTEGER)
    DIM state AS LONG, total AS LONG, digit AS LONG, i AS INTEGER
    state = 1
    total = 0
    FOR i = 1 TO n
        state = (state * 75 + 74) MOD 65537
        digit = state MOD 10
        total = total + Weight&(state MOD 14) + Route&(digit * digit * 37)
    NEXT i
    BenchSwitch& = total
END FUNCTION

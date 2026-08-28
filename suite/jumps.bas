' ON GOTO and SELECT CASE, which compile to FF /4 and FF /5 -- indirect jumps
' whose targets are not computable from the instruction, and are three
' consecutive offset16 fixups into the module's own code segment.
DEFINT A-Z
DIM a AS LONG, b AS LONG, r AS LONG

a = 305419896
b = 252645135

FOR k = 1 TO 3
    ON k GOTO one, two, three
one:
    r = a AND b
    GOTO done
two:
    r = a OR b
    GOTO done
three:
    r = a XOR b
done:
    PRINT "ON"; k; "="; r

    SELECT CASE k
        CASE 1
            r = a + b
        CASE 2
            r = a - b
        CASE ELSE
            r = -a
    END SELECT
    PRINT "CASE"; k; "="; r
NEXT k
PRINT "DONE"

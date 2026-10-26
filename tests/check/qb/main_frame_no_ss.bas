' RUN: llrm-qb %s --dialect qb45 --runtime qb45 -Os -S -o /dev/stdout
' The main body's static frame is reached through ds: nothing in it writes ds, so
' no cell carries an `ss:` prefix (pow_unrolled: 7 extra bytes, pow_stack_depth: 52).
' CHECK-LABEL: QB$MAIN proc
' CHECK: fstp tbyte ptr $QB$FRAME
' CHECK-NOT: ss:
' CHECK: QB$MAIN endp
DIM d AS DOUBLE
FOR i% = -7 TO 7
    d = 99999999999999.95# * 10# ^ i%
    PRINT d
NEXT

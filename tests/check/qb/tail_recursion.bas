' RUN: llrm-qb %s --dialect {qb45 | pds71 | vbdos} --runtime {qb45 | pds71 | vbdos} -O2 -march=i486 -fno-inline-functions -S -o /dev/stdout
' `n + Sum%(n - 1)` was a call per level; with its argument BYVAL it is a loop. By reference (the default)
' the argument is the address of a frame cell the callee reads, and the call stays.
' CHECK-LABEL: SUM proc
' CHECK-NOT: call
' CHECK: SUM endp
' CHECK-LABEL: SUMREF proc
' CHECK: call
' CHECK: SUMREF endp
DECLARE FUNCTION Sum% (BYVAL n AS INTEGER)
DECLARE FUNCTION SumRef% (n AS INTEGER)
PRINT Sum%(10); SumRef%(10)

FUNCTION Sum% (BYVAL n AS INTEGER)
  IF n = 0 THEN
    Sum% = 0
  ELSE
    Sum% = n + Sum%(n - 1)
  END IF
END FUNCTION

FUNCTION SumRef% (n AS INTEGER)
  IF n = 0 THEN
    SumRef% = 0
  ELSE
    SumRef% = n + SumRef%(n - 1)
  END IF
END FUNCTION

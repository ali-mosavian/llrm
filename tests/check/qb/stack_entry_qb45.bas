' RUN: llrm-qb %s --dialect qb45 --runtime qb45 -fsanitize=stack -S -o /dev/stdout
' A procedure the runtime frames enters through its checking entry, B$ENRD, as BC /D does,
' and carries no check of its own.
' CHECK-LABEL: DEEP proc
' CHECK-NOT: b$pendchk
' CHECK: call far ptr B$ENRD
' CHECK-NOT: B$ENRA
' CHECK: DEEP endp
DECLARE FUNCTION Deep% (n AS INTEGER)
PRINT Deep%(3)
FUNCTION Deep% (n AS INTEGER)
  IF n = 0 THEN
    Deep% = 0
  ELSE
    Deep% = 1 + Deep%(n - 1)
  END IF
END FUNCTION

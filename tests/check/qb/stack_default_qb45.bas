' RUN: llrm-qb %s --dialect qb45 --runtime qb45 --own-frames -S -o /dev/stdout
' The default build checks nothing: no limit word, no overflow routine.
' CHECK-LABEL: DEEP proc
' CHECK-NOT: b$pendchk
' CHECK-NOT: B$ERR_OSS
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

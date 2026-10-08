' RUN: llrm-qb %s --dialect pds71 --runtime pds71 --own-frames -fsanitize=stack -S -o /dev/stdout
' -fsanitize=stack: the procedure compares SP with the runtime's limit word once its frame is
' allocated, and calls the runtime's overflow routine out of line, last.
' CHECK-LABEL: DEEP proc
' CHECK: sub sp, 2
' CHECK-NEXT: jb
' CHECK-NEXT: cmp sp, word ptr b$pendchk
' CHECK-NEXT: jb
' CHECK: call far ptr B$ERR_OSS
' CHECK-NEXT: DEEP endp
DECLARE FUNCTION Deep% (n AS INTEGER)
PRINT Deep%(3)
FUNCTION Deep% (n AS INTEGER)
  IF n = 0 THEN
    Deep% = 0
  ELSE
    Deep% = 1 + Deep%(n - 1)
  END IF
END FUNCTION

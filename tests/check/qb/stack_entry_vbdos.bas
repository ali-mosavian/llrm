' RUN: llrm-qb %s --dialect vbdos --runtime vbdos -fsanitize=stack -S -o /dev/stdout
' A procedure the runtime frames (a local STRING needs its handle) enters through the checking
' entry, B$ENRD, as BC /D does, and carries no check of its own, and no shell of its own: the entry builds the frame.
' CHECK-LABEL: S proc
' CHECK-NOT: push bp
' CHECK-NOT: b$pendchk
' CHECK: call far ptr B$ENRD
' CHECK-NOT: B$ENRA
' CHECK: S endp
S
SUB S
  DIM t AS STRING
  t = "x"
  PRINT t
END SUB

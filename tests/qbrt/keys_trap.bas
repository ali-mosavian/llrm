' ON KEY(n) GOSUB: a trapped key is taken from the keyboard and runs its
' handler between statements; any other key is read by INKEY$ as usual.
' Played by test_qbkeys.py, which types F1, F2, a, F1.
DEFINT A-Z
DIM SHARED f1, f2
ON KEY(1) GOSUB one
ON KEY(2) GOSUB two
KEY(1) ON
KEY(2) ON
PRINT "ready"
other$ = ""
DO WHILE f1 + f2 < 3
  k$ = INKEY$
  IF k$ <> "" THEN other$ = other$ + k$
LOOP
KEY(1) OFF
KEY(2) OFF
PRINT "f1 ="; f1; "f2 ="; f2; "other = "; other$
PRINT "done"
END

one:
  f1 = f1 + 1
  RETURN

two:
  f2 = f2 + 1
  RETURN

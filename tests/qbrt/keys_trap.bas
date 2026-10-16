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
' Each phase has one reader of the keyboard, so a key's fate does not depend on the instant it arrives: the trap looks before each
' statement, INKEY$ reads in the middle of one, and a key that arrived between the two went to INKEY$ and was never trapped.
DO WHILE f1 < 1
LOOP
DO WHILE f2 < 1
LOOP
DO
  other$ = INKEY$
LOOP WHILE other$ = ""
DO WHILE f1 < 2
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

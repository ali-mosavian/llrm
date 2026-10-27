' INT((x + c) / n) with a SINGLE x kept the quotient of x alone: x = 3 printed 1, not 2.
' Each line is one case; the last line is DONE.
x = 3
y = 0.5
z = -3
PRINT INT((x + 1) / 2); INT((x - 1) / 2); INT((x + 1) / 4)
PRINT INT((y + 1) / 2); INT((z + 1) / 2); INT((z - 1) / 4)
DONE$ = "DONE"
PRINT DONE$
END

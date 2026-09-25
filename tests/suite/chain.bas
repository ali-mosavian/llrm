' A long divide whose second operand is computed by another long divide.
'
' Absorption emits each call site on its own, so the outer operand is pushed,
' the whole inner call runs, and only then is the outer one popped back. After
' absorption that reads
'
'     push bx / push cx      the outer divisor, split into BC's halves
'     ...                    the inner divide, ending in
'     pop ecx                its own divisor
'     idiv ecx
'     pop ecx                the rejoin -- and the outer divide wanted this
'     idiv ecx
'
' and the two pushes and the rejoin look like a round trip that computes
' nothing, which they are. Deleting them is still wrong: the pop in the middle
' has overwritten ecx, so the value is no longer where the argument for
' deleting it says it is. That deletion shipped, and it made the second divide
' use the first one's divisor.
'
' `x XOR x` is here because it is how a fuzzed program built a divisor of 1 --
' any expression would do, and this one makes the right answer easy to see.
DEFINT A-Z
DIM a AS LONG, b AS LONG, c AS LONG, d AS LONG
DIM r AS LONG

a = 1073741831
b = 39678839
c = -1049330653
d = 100003

r = (a MOD b) MOD (((c XOR c) AND 2147483647) OR 1)
PRINT "ONE="; r

' The shape a fuzzed program actually miscompiled: the inner divisor is a
' constant, so BC has the outer one in a register pair before the inner call
' runs at all.
r = (a MOD 39678839) MOD (((c XOR c) AND 2147483647) OR 1)
PRINT "CONST="; r

r = (a MOD 39678839) MOD ((d AND 2147483647) OR 1)
PRINT "CONST2="; r

r = (a MOD b) MOD ((d AND 2147483647) OR 1)
PRINT "MODMOD="; r

r = (a \ b) \ (((c XOR c) AND 32767) OR 3)
PRINT "DIVDIV="; r

a = -1073741831
r = (a MOD b) MOD ((d AND 2147483647) OR 1)
PRINT "NEGMOD="; r

r = (a \ b) \ (((c XOR c) AND 32767) OR 3)
PRINT "NEGDIV="; r

PRINT "DONE"
END

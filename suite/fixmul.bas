' fixMul& has no body anywhere -- it is declared and never defined, and qbopt
' owns the call site entirely. If a region absorbing it were refused, LINK
' would fail on the unresolved external, so this is a stronger check than the
' three-way differential: there is no base build that can run at all, and
' e2e.py is not used here for exactly that reason.
'
' (int32)(((int64) a * b) >> fixShift). N.M times N.M is N.2M, which does not
' fit back in 32 bits without the shift that undoes the doubled fraction, and
' the width M is the caller's format to choose -- so it is a third argument,
' not a constant baked into the pass. It is almost always known at compile
' time, and then it is one byte of an immediate; m7 passes it in a variable to
' prove the other path, loading it into cl, still works.
'
' fixShift is `as long` only so it reaches the stack the same way a and b do.
'
' Five value cases at a 16-bit shift: two positive, two negative, one of each,
' the extremes, and a constant right operand, which has no immediate form of
' imul and needs its own load. m6 repeats the first case at an 8-bit shift, an
' 8.24 format instead of 16.16, to prove the shift is not fixed.
'
' Calls and prints alternate in two groups rather than one straight run: BC
' flushes a LEDATA record roughly every 128 bytes regardless of what is inside
' it, and a call whose pushes and call instruction land in different LEDATA
' records is refused rather than risked. Seven calls in one unbroken run
' crossed that boundary on QuickBASIC 4.5; two shorter runs do not.
declare function fixMul& (byval a as long, byval b as long, byval fixShift as long)
defint a-z
dim valA as long
dim valB as long
dim shiftVar as long
dim m1 as long
dim m2 as long
dim m3 as long
dim m4 as long
dim m5 as long
dim m6 as long
dim m7 as long

valA = 65536
valB = 131072
m1 = fixMul&(valA, valB, 16)

valA = -65536
m2 = fixMul&(valA, valB, 16)

valA = -65536
valB = -131072
m3 = fixMul&(valA, valB, 16)

valA = 2147483647
m4 = fixMul&(valA, 2, 16)

valA = -2147483648
valB = 65536
m5 = fixMul&(valA, valB, 16)

valA = 65536
valB = 131072
print "M1="; m1
print "M2="; m2
print "M3="; m3
print "M4="; m4
print "M5="; m5

m6 = fixMul&(valA, valB, 8)

shiftVar = 16
m7 = fixMul&(valA, valB, shiftVar)

print "M6="; m6
print "M7="; m7
print "DONE"
end

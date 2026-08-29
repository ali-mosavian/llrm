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
' 8.24 format instead of 16.16, to prove the shift is not fixed. Each result
' prints twice, the raw long and the value it means as a double, read straight
' off BASIC's own PRINT rather than guessed at.
declare function fixMul& (byval a as long, byval b as long, byval fixShift as long)
defint a-z
dim valA as long
dim valB as long
dim shiftVar as long
dim result as long

valA = 65536
valB = 131072
result = fixMul&(valA, valB, 16)
print "M1="; result
print "F1="; cdbl(result) / 65536

valA = -65536
result = fixMul&(valA, valB, 16)
print "M2="; result
print "F2="; cdbl(result) / 65536

valA = -65536
valB = -131072
result = fixMul&(valA, valB, 16)
print "M3="; result
print "F3="; cdbl(result) / 65536

valA = 2147483647
result = fixMul&(valA, 2, 16)
print "M4="; result
print "F4="; cdbl(result) / 65536

valA = -2147483648
valB = 65536
result = fixMul&(valA, valB, 16)
print "M5="; result
print "F5="; cdbl(result) / 65536

valA = 65536
valB = 131072
result = fixMul&(valA, valB, 8)
print "M6="; result
print "F6="; cdbl(result) / 256

shiftVar = 16
result = fixMul&(valA, valB, shiftVar)
print "M7="; result
print "F7="; cdbl(result) / 65536

print "DONE"
end

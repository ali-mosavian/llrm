' fixMul& has no body anywhere -- it is declared and never defined, and qbopt
' owns the call site entirely. If a region absorbing it were refused, LINK
' would fail on the unresolved external, so this is a stronger check than the
' three-way differential: there is no base build that can run at all, and
' e2e.py is not used here for exactly that reason.
'
' (int32)(((int64) a * b) >> 16), the 64-bit shift a 16.16 multiply means. Five
' cases: two positive, two negative, one of each, the extremes, and a constant
' right operand, which has no immediate form of imul and needs its own load.
'
' Every call is made before any PRINT: BC starts a new LEDATA record around a
' PRINT statement, and a call whose pushes and call instruction land in
' different LEDATA records is refused rather than risked -- interleaving
' prints between the cases hit exactly that on PDS and QuickBASIC 4.5.
declare function fixMul& (byval a as long, byval b as long)
defint a-z
dim valA as long
dim valB as long
dim m1 as long
dim m2 as long
dim m3 as long
dim m4 as long
dim m5 as long

valA = 65536
valB = 131072
m1 = fixMul&(valA, valB)

valA = -65536
m2 = fixMul&(valA, valB)

valA = -65536
valB = -131072
m3 = fixMul&(valA, valB)

valA = 2147483647
m4 = fixMul&(valA, 2)

valA = -2147483648
valB = 65536
m5 = fixMul&(valA, valB)

print "M1="; m1
print "M2="; m2
print "M3="; m3
print "M4="; m4
print "M5="; m5
print "DONE"
end

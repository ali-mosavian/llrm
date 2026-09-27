dim integerValue as integer
dim longValue as long
dim singleValue as single
dim doubleValue as double
dim dividend as integer
dim divisor as integer
dim longFactor as long
dim longMultiplier as long
dim longAddend as long
dim firstSingle as single
dim secondSingle as single

read dividend, divisor, longFactor, longMultiplier, longAddend, firstSingle, secondSingle
print "READ"
integerValue = dividend \ divisor
print "IDIV"
longValue = longFactor * longMultiplier + longAddend
print "LONG"
singleValue = csng(firstSingle) + csng(secondSingle)
print "SINGLE"
doubleValue = cdbl(singleValue) / cdbl(2)
print "DOUBLE"
end

data 17, 3, 100000, 3, 7, 1, 2

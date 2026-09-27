' QB45 compatibility source.
dim integerValue as integer
dim longValue as long
dim singleValue as single
dim doubleValue as double
dim passed as integer
dim dividend as integer
dim divisor as integer
dim longFactor as long
dim longMultiplier as long
dim longAddend as long
dim firstSingle as single
dim secondSingle as single

read dividend, divisor, longFactor, longMultiplier, longAddend, firstSingle, secondSingle
integerValue = dividend \ divisor
longValue = longFactor * longMultiplier + longAddend
singleValue = csng(firstSingle) + csng(secondSingle)
doubleValue = cdbl(singleValue) / cdbl(2)
passed = integerValue = 5
if passed then passed = longValue = 300007
if passed then passed = singleValue = csng(3)
if passed then passed = doubleValue = cdbl(1.5)

if passed then
    print "PASS numeric"
else
    print "FAIL numeric conversions"
end if
end

data 17, 3, 100000, 3, 7, 1, 2

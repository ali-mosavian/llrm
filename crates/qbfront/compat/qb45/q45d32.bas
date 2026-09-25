' DEFtype, DEF FN, EXIT, and SWAP language forms.
defint a-z
def fn twice(value) = value * 2
dim firstValue
dim secondValue
dim index
dim total

firstValue = fn twice(4)
secondValue = 3
swap firstValue, secondValue
for index = 1 to 10
    total = total + 1
    if index = 3 then exit for
next index

if firstValue = 3 and secondValue = 8 and total = 3 then
    print "PASS definitions"
else
    print "FAIL definitions forms"
end if
end

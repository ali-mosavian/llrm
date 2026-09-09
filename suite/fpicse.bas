' Runtime integer conversions shared across statements, without fast-math.
dim inputValue as long
dim sample as integer
dim firstValue as double
dim secondValue as double
data -32768, 123, 32767
for sample = 1 to 3
    read inputValue
    firstValue = inputValue
    secondValue = inputValue
    print "VALUE="; clng(firstValue); clng(secondValue)
next sample
print "DONE"
end

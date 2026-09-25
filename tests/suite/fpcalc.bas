' Convert a computed runtime integer twice; it has no source memory cell.
dim inputValue as integer
dim sample as integer
dim firstValue as double
dim secondValue as double
data -32768, 123, 32766
for sample = 1 to 3
    read inputValue
    firstValue = inputValue + 1
    secondValue = inputValue + 1
    print "VALUE="; clng(firstValue); clng(secondValue)
next sample
print "DONE"
end

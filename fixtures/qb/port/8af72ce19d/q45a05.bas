' QB45 compatibility source.
option base 1
dim row as integer
dim column as integer
dim values(1 to 2, 1 to 3) as integer
dim passed as integer

for row = 1 to 2
    for column = 1 to 3
        values(row, column) = row * 10 + column
    next column
next row

passed = values(1, 1) = 11 and values(1, 2) = 12 and values(1, 3) = 13
passed = passed and values(2, 1) = 21 and values(2, 2) = 22 and values(2, 3) = 23
passed = passed and lbound(values, 1) = 1 and ubound(values, 1) = 2
passed = passed and lbound(values, 2) = 1 and ubound(values, 2) = 3
if passed then
    print "PASS arrays"
else
    print "FAIL arrays bounds"
end if
end

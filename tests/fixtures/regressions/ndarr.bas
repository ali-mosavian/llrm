' $dynamic
dim items(-1 to 0, 2 to 4, 1 to 1, 1 to 1, 1 to 1, 1 to 1, 1 to 1, 1 to 1, 5 to 6) as integer
dim rowIndex as integer
dim colIndex as integer
dim pageIndex as integer
dim value as integer
value = 0
for pageIndex = 5 to 6
    for colIndex = 2 to 4
        for rowIndex = -1 to 0
            value = value + 1
            items(rowIndex, colIndex, 1, 1, 1, 1, 1, 1, pageIndex) = value
        next rowIndex
    next colIndex
next pageIndex
print items(-1, 2, 1, 1, 1, 1, 1, 1, 5); items(0, 4, 1, 1, 1, 1, 1, 1, 6); items(0, 2, 1, 1, 1, 1, 1, 1, 5)
print "DONE"

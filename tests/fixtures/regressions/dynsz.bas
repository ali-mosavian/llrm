rem $dynamic
dim rowCount as integer
dim columnCount as integer
data 3, 5
read rowCount, columnCount
dim items(-2 to rowCount, 4 to columnCount) as integer
items(-1, 5) = 123
items(3, 4) = 456
print items(-1, 5); items(3, 4)
print "DONE"
end

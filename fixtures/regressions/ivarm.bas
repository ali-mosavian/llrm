dim branchChoice as long
dim stepCount as integer
dim currentValue as integer
dim values(0 to 1) as integer
data 1
read branchChoice
currentValue = 7
for stepCount = 1 to 10
    if branchChoice then
        values(0) = currentValue
    else
        values(1) = currentValue
    end if
    currentValue = currentValue + 3
next stepCount
print values(0); values(1); stepCount; currentValue
print "DONE"

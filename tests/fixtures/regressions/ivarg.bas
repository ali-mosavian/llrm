declare sub chooseField (branchChoice as long)
dim branchChoice as long
data 1
read branchChoice
call chooseField(branchChoice)
print "DONE"

sub chooseField (branchChoice as long)
    dim firstValue as integer, secondValue as integer
    dim stepCount as integer, currentValue as integer
    firstValue = 0
    secondValue = 0
    currentValue = 7
    for stepCount = 1 to 10
        if branchChoice then
            firstValue = currentValue
        else
            secondValue = currentValue
        end if
        currentValue = currentValue + 3
    next stepCount
    print firstValue; secondValue; stepCount; currentValue
end sub

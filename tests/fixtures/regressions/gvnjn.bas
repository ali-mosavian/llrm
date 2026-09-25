dim inputValue as long
dim branchChoice as integer
dim trial as integer
dim answer as long
dim square as long
data 5, 0, 5, 1
for trial = 1 to 2
    read inputValue, branchChoice
    inputValue = inputValue + 1
    if branchChoice then
        answer = inputValue * inputValue + 1
    else
        answer = inputValue * inputValue - 1
    end if
    square = inputValue * inputValue
    print answer; square
next trial
print "DONE"

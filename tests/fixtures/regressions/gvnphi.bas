dim inputValue as long
dim branchChoice as integer
dim trial as integer
dim answer as long
dim square as long
data 5, 0, 5, 1
for trial = 1 to 2
    read inputValue, branchChoice
    if branchChoice then
        inputValue = inputValue + 1
        answer = inputValue * inputValue + 1
    else
        inputValue = inputValue + 2
        answer = inputValue * inputValue - 1
    end if
    square = inputValue * inputValue
    print answer; square
next trial
print "DONE"

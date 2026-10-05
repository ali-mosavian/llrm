dim inputValue as integer
dim branchChoice as integer
dim answer as integer
data 5, 0, 5, 1
read inputValue, branchChoice
dim firstValues(1 to 4) as integer
if branchChoice then
    firstValues(2) = inputValue + 1
else
    firstValues(2) = inputValue + 2
end if
answer = firstValues(2) + 3
print answer
read inputValue, branchChoice
dim secondValues(1 to 4) as integer
if branchChoice then
    secondValues(2) = inputValue + 1
else
    secondValues(2) = inputValue + 2
end if
answer = secondValues(2) + 3
print answer
print "DONE"

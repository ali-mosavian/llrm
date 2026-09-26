declare function ordered& (leftValue as long, byval middleValue as long, byval rightValue as long)

dim answer as long
dim leftValue as long
dim middleValue as long
dim rightValue as long

leftValue = 50
middleValue = 8
rightValue = 3
answer = ordered(leftValue, middleValue, rightValue)
if answer = 39 and leftValue = 150 and middleValue = 8 and rightValue = 3 then
    print "PASS pds-quick-call"
else
    print "FAIL pds-quick-call result"
end if
end

function ordered& (leftValue as long, byval middleValue as long, byval rightValue as long)
    ordered = leftValue - middleValue * 4 + rightValue * 7
    leftValue = leftValue + 100
    middleValue = middleValue + 1
    rightValue = rightValue + 1
end function

' Pre/post DO conditions and signed FOR steps have separate witnesses.
dim limitValue as integer
dim loopValue as integer
dim preWhileCount as integer
dim postWhileCount as integer
dim preUntilCount as integer
dim postUntilCount as integer
dim exitDoCount as integer
dim upTrace as integer
dim downTrace as integer
dim skippedCount as integer

read limitValue
loopValue = 0
do while loopValue < limitValue
    preWhileCount = preWhileCount + 1
    loopValue = loopValue + 1
loop
loopValue = limitValue
do
    postWhileCount = postWhileCount + 1
loop while loopValue < limitValue
loopValue = limitValue
do until loopValue >= limitValue
    preUntilCount = preUntilCount + 1
loop
loopValue = limitValue
do
    postUntilCount = postUntilCount + 1
loop until loopValue >= limitValue
do
    exitDoCount = exitDoCount + 1
    exit do
loop
for loopValue = 1 to limitValue step 1
    upTrace = upTrace * 10 + loopValue
next loopValue
for loopValue = limitValue to 1 step -1
    downTrace = downTrace * 10 + loopValue
next loopValue
for loopValue = limitValue to 1 step 1
    skippedCount = skippedCount + 1
next loopValue
if preWhileCount <> 3 then
    print "FAIL loopforms pre-while"
    end
end if
if postWhileCount <> 1 then
    print "FAIL loopforms post-while"
    end
end if
if preUntilCount <> 0 then
    print "FAIL loopforms pre-until"
    end
end if
if postUntilCount <> 1 then
    print "FAIL loopforms post-until"
    end
end if
if exitDoCount <> 1 then
    print "FAIL loopforms exit-do"
    end
end if
if upTrace <> 123 then
    print "FAIL loopforms for-up"
    end
end if
if downTrace <> 321 then
    print "FAIL loopforms for-down"
    end
end if
if skippedCount <> 0 then
    print "FAIL loopforms for-skip"
    end
end if
print "PASS loopforms"
end

data 3

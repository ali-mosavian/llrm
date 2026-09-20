' QB45 compatibility source.
dim index as integer
dim forTotal as integer
dim doTotal as integer
dim whileTotal as integer
dim choice as integer

for index = 1 to 4
    forTotal = forTotal + index
next index
if forTotal <> 10 then
    print "FAIL control for"
    end
end if

doTotal = 7
do while doTotal < 10
    doTotal = doTotal + 1
loop
if doTotal <> 10 then
    print "FAIL control do"
    end
end if

whileTotal = 4
while whileTotal < 7
    whileTotal = whileTotal + 1
wend
if whileTotal <> 7 then
    print "FAIL control while"
    end
end if

if forTotal = 10 then
    choice = 1
elseif forTotal = 0 then
    choice = 2
else
    choice = 3
end if

select case choice
case 1
    if doTotal = 10 and whileTotal = 7 then
        print "PASS control"
    else
        print "FAIL control checkpoint"
    end if
case else
    print "FAIL control branch"
end select
end

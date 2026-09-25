' QB45 compatibility source.
declare sub increase (argData as integer)
declare function addLong& (firstArg as long, secondArg as long)

dim count as integer
dim total as long

count = 4
call increase(count)
total = addLong&(100000, 23)

if count = 5 and total = 100023 then
    print "PASS procedures"
else
    print "FAIL procedures byref"
end if
end

sub increase (argData as integer)
    argData = argData + 1
end sub

function addLong& (firstArg as long, secondArg as long)
    addLong& = firstArg + secondArg
end function

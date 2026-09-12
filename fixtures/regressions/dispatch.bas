dim selector as integer
read selector
on selector goto firstCase, secondCase, thirdCase
print "DEFAULT"
goto finished
firstCase:
print "FIRST"
goto finished
secondCase:
print "SECOND"
goto finished
thirdCase:
print "THIRD"
finished:
print "DONE"
end
data 256

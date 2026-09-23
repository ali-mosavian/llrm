' QB45 compatibility source.
dim index as integer
dim total as integer

gosub firstPart
index = 2
on index gosub firstPart, secondPart

if total = 30 then
    goto passed
end if
print "FAIL gotos dispatch"
end

firstPart:
total = total + 10
return

secondPart:
total = total + 20
return

passed:
print "PASS gotos"
end

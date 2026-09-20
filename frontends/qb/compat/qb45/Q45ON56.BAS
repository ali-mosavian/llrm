' ON dispatch checks every GOSUB target and a noncommutative GOTO target.
dim firstChoice as integer
dim secondChoice as integer
dim thirdChoice as integer
dim lowChoice as integer
dim highChoice as integer
dim routeTrace as integer
dim returnCount as integer
dim gotoWitness as integer

read firstChoice, secondChoice, thirdChoice, lowChoice, highChoice
on firstChoice gosub routeOne, routeTwo, routeThree
returnCount = returnCount + 1
on secondChoice gosub routeOne, routeTwo, routeThree
returnCount = returnCount + 1
on thirdChoice gosub routeOne, routeTwo, routeThree
returnCount = returnCount + 1
on lowChoice gosub routeOne, routeTwo, routeThree
returnCount = returnCount + 1
on highChoice gosub routeOne, routeTwo, routeThree
returnCount = returnCount + 1
on secondChoice goto gotoOne, gotoTwo, gotoThree
print "FAIL on-dispatch goto-fallthrough"
end

gotoOne:
print "FAIL on-dispatch goto-one"
end

gotoTwo:
gotoWitness = 37
goto afterGoto

gotoThree:
print "FAIL on-dispatch goto-three"
end

afterGoto:
if routeTrace <> 123 or returnCount <> 5 or gotoWitness <> 37 then
    print "FAIL on-dispatch checkpoints"
    end
end if
print "PASS on-dispatch"
end

routeOne:
routeTrace = routeTrace * 10 + 1
return

routeTwo:
routeTrace = routeTrace * 10 + 2
return

routeThree:
routeTrace = routeTrace * 10 + 3
return

data 1, 2, 3, 0, 4

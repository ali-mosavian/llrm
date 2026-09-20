' SELECT CASE list, range boundaries, relational form, and ELSE are isolated.
dim listValue as integer
dim rangeLow as integer
dim rangeHigh as integer
dim relationValue as integer
dim elseValue as integer
dim listWitness as integer
dim lowWitness as integer
dim highWitness as integer
dim relationWitness as integer
dim elseWitness as integer

read listValue, rangeLow, rangeHigh, relationValue, elseValue
select case listValue
case 1, 2, 3
    listWitness = 11
case else
    listWitness = -1
end select
select case rangeLow
case 4 to 6
    lowWitness = 21
case else
    lowWitness = -1
end select
select case rangeHigh
case 4 to 6
    highWitness = 22
case else
    highWitness = -1
end select
select case relationValue
case is > 6
    relationWitness = 31
case else
    relationWitness = -1
end select
select case elseValue
case 1 to 9
    elseWitness = -1
case else
    elseWitness = 41
end select
if listWitness <> 11 then
    print "FAIL selectforms list"
    end
end if
if lowWitness <> 21 then
    print "FAIL selectforms range-low"
    end
end if
if highWitness <> 22 then
    print "FAIL selectforms range-high"
    end
end if
if relationWitness <> 31 then
    print "FAIL selectforms relation"
    end
end if
if elseWitness <> 41 then
    print "FAIL selectforms else"
    end
end if
print "PASS selectforms"
end

data 2, 4, 6, 7, -1

defint a-z

declare function sumThree (first() as integer, second() as integer, third() as integer) as integer

dim first(0 to 3) as integer
dim second(0 to 3) as integer
dim third(0 to 3) as integer

first(0) = 1
first(1) = 2
first(2) = 3
first(3) = 4
second(0) = 10
second(1) = 20
second(2) = 30
second(3) = 40
third(0) = 100
third(1) = 200
third(2) = 300
third(3) = 400

print "RESULT="; sumThree(first(), second(), third())
print "DONE"
end

function sumThree (first() as integer, second() as integer, third() as integer) as integer static
    dim index as integer
    dim total as integer

    total = 0
    for index = lbound(first) to ubound(first)
        total = total + first(index)
        total = total + second(index)
        total = total + third(index)
    next index
    sumThree = total
end function

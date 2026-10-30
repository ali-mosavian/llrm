defint a-z

declare function sumThree (first() as integer, snd() as integer, third() as integer) as integer

dim first(0 to 3) as integer
dim snd(0 to 3) as integer
dim third(0 to 3) as integer

first(0) = 1
first(1) = 2
first(2) = 3
first(3) = 4
snd(0) = 10
snd(1) = 20
snd(2) = 30
snd(3) = 40
third(0) = 100
third(1) = 200
third(2) = 300
third(3) = 400

print "RESULT="; sumThree(first(), snd(), third())
print "DONE"
end

function sumThree (first() as integer, snd() as integer, third() as integer) as integer static
    dim index as integer
    dim total as integer

    total = 0
    for index = lbound(first) to ubound(first)
        total = total + first(index)
        total = total + snd(index)
        total = total + third(index)
    next index
    sumThree = total
end function

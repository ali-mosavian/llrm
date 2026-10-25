defint a-z

declare function sumThree% (first() as integer, snd() as integer, third() as integer)
declare function BenchSumThree% ()

dim shared first(0 to 3) as integer
dim shared snd(0 to 3) as integer
dim shared third(0 to 3) as integer

print ltrim$(str$(BenchSumThree%))
end

function sumThree% (first() as integer, snd() as integer, third() as integer) static
    dim index as integer
    dim total as integer

    total = 0
    for index = lbound(first) to ubound(first)
        total = total + first(index)
        total = total + snd(index)
        total = total + third(index)
    next index
    sumThree% = total
end function

function BenchSumThree%
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
    BenchSumThree% = sumThree%(first(), snd(), third())
end function

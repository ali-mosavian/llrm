defint a-z

declare function parityControl (value as integer, limit as integer) as long
declare function BenchControl& ()

print ltrim$(str$(BenchControl&()))
end

function parityControl (value as integer, limit as integer) as long
    dim total as long
    dim index as integer

    total = 0
    for index = 0 to limit - 1
        if (index and 1) = 0 then
            total = total + clng(value) + index
        else
            total = total - clng(value) + index
        end if
    next index
    parityControl = total
end function

function BenchControl& ()
    BenchControl& = parityControl(7, 6) * 1000 + parityControl(-3, 5)
end function

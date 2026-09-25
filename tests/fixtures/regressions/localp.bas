' A procedure-local loop accumulator should survive across statements.
declare sub accumulate (limit as integer)
dim limit as integer
read limit
call accumulate(limit)
print "DONE"
data 7

sub accumulate (limit as integer)
    dim total as long, index as integer
    total = 0
    for index = 1 to limit
        total = total + index
    next index
    print total
end sub

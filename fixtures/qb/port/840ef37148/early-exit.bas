function classify (byval value as integer) as integer
    if value < 0 then classify = -1: exit function
    classify = 1
end function

' The true arm supplies x; only the false arm needs to load it at the join.
dim choice as integer, sample as integer
dim x as long, y as long, answer as long
for sample = 1 to 3
    read choice, x, y
    if choice then
        x = y + 1
    else
        y = y + 2
    end if
    answer = x * 7
    print answer
next sample
print "DONE"
data 1, 10, 4, 0, 10, 4, -1, -3, -5

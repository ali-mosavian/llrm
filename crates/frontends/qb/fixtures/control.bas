dim shared total as long
dim i as integer

for i = 1 to 8
    if i mod 2 = 0 then
        total = total + i
    end if
next i

do while total < 40
    total = total + 1
loop

finished:
if total = 40 then goto finished

dim values(0 to 3) as integer
dim index as integer
dim factor as integer
dim total as integer
data 7
read factor
for index = 0 to 9
    if index < 4 then
        values(index) = factor + index
        total = total + factor
    end if
next index
print total; values(0); values(3)
print "DONE"

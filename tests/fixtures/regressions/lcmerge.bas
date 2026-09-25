dim stopAt as integer
dim counter as integer
dim total as long
data 5
read stopAt
counter = 0
total = 0
do while counter < 10
    if counter = stopAt then exit do
    total = total + counter
    counter = counter + 1
loop
total = total + 1
print "COUNT="; counter
print "TOTAL="; total
print "DONE"
end

' $dynamic
dim items(0 to 200, -2 to 198) as integer
dim stepCount as integer
for stepCount = 0 to 1
    items(4 + stepCount, 161) = 456 + stepCount
    items(163, 2 + stepCount) = 789 + stepCount
next stepCount
print items(4, 161); items(5, 161); items(163, 2); items(163, 3)
print "DONE"

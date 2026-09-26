' $dynamic
dim items(0 to 200, -2 to 198) as integer
items(0, -2) = 123
items(4, 161) = 456
items(5, 161) = 789
items(163, 2) = 111
items(163, 3) = 222
print items(0, -2); items(4, 161); items(5, 161); items(163, 2); items(163, 3)
print "DONE"

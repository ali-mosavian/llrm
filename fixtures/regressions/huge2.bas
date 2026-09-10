' $dynamic
dim items(0 to 200, -2 to 198) as integer
items(0, -2) = 123
items(4, 161) = 456
items(5, 161) = 789
print items(0, -2); items(4, 161); items(5, 161)
print "DONE"

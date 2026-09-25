' Requires /Ah. The 201 by 201 INTEGER object crosses one 64 KiB segment.
' $dynamic

dim items(0 to 200, -2 to 198) as integer
dim passed as integer

items(0, -2) = 123
items(4, 161) = 456
items(5, 161) = 789
items(163, 2) = 111
items(163, 3) = 222

passed = items(0, -2) = 123 and items(4, 161) = 456
passed = passed and items(5, 161) = 789
passed = passed and items(163, 2) = 111 and items(163, 3) = 222

if passed then
    print "PASS pds-huge-array"
else
    print "FAIL pds-huge-array indexing"
end if
end

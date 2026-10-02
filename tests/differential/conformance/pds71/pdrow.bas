dim items(0 to 1, 0 to 1) as integer
dim passed as integer

items(0, 0) = 1
items(1, 0) = 2
items(0, 1) = 3
items(1, 1) = 4
def seg = varseg(items(0, 0))
passed = peek(varptr(items(0, 0))) = 1 and peek(varptr(items(0, 0)) + 2) = 3
def seg

if passed then
    print "PASS pds-row-major-array"
else
    print "FAIL pds-row-major-array layout"
end if
end

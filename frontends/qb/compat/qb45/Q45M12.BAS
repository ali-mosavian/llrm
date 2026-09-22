' QB45 compatibility source.
dim values(0 to 2) as integer
dim byteValue as integer

values(0) = &H1111
values(1) = &H1234
values(2) = &H2222
def seg = varseg(values(1))
byteValue = peek(varptr(values(1)))
poke varptr(values(1)), &H56
def seg

if byteValue = &H34 and values(1) = &H1256 and values(0) = &H1111 and values(2) = &H2222 then
    print "PASS memory"
else
    print "FAIL memory segment"
end if
end

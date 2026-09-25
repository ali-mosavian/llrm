declare sub report (tag as string, n as long)

dim n as long
n = 1
call report("STACK", n)

sub report (tag as string, n as long)
    dim localBuffer as string * 4096
    localBuffer = "X"
    print "STACK RESERVED"
end sub

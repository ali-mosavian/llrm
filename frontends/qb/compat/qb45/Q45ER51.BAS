' A DATA-fed zero divisor must raise exactly BASIC error 11 at line 100.
dim divisor as integer
dim quotient as integer
dim resumed as integer

on error goto handler
read divisor
100 quotient = 7 \ divisor
resumed = 1
if resumed <> 1 then
    print "FAIL divzero resume"
    end
end if
print "PASS divzero"
end

handler:
if err <> 11 then
    print "FAIL divzero errnum"
    end
end if
if erl <> 100 then
    print "FAIL divzero errline"
    end
end if
resume next

data 0

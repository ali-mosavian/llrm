' A DATA-fed out-of-range index must raise exactly error 9 at line 100.
dim values(1 to 3) as integer
dim indexValue as integer
dim observed as integer
dim resumed as integer

on error goto handler
values(1) = 17
values(2) = 23
values(3) = 29
read indexValue
100 observed = values(indexValue)
resumed = 1
if values(1) <> 17 or values(2) <> 23 or values(3) <> 29 or resumed <> 1 then
    print "FAIL bounds resume"
    end
end if
print "PASS bounds-error"
end

handler:
if err <> 9 then
    print "FAIL bounds errnum"
    end
end if
if erl <> 100 then
    print "FAIL bounds errline"
    end
end if
resume next

data 4

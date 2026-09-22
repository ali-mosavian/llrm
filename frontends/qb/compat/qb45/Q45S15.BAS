' QB45 compatibility source.
declare function qCount (qInput as integer)
dim qFirst as integer
dim qSecond as integer

qFirst = qCount(0)
qSecond = qCount(0)
if qFirst = 1 and qSecond = 2 then
    print "PASS static"
else
    print "FAIL static lifetime"
end if
end

function qCount (qInput as integer)
    static qTally as integer
    qTally = qTally + 1
    qCount = qTally + qInput
end function
